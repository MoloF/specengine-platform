import { describe, expect, it } from "vitest";
import { ARC_BEND, arcOver, COLUMN_GAP, FIT_MARGIN, FIT_MAX_ZOOM, FIT_MIN_ZOOM, readableViewport } from "./geometry";
import { BOX_HEIGHT, BOX_WIDTH, COLUMN_STEP, ROW_STEP } from "./layout";

// The canvas's geometry (docs/features/ui-graph.md "Canvas"), as numbers jsdom cannot draw: a
// link within one column swings through the gap on its right, its label there; a fit never shrinks
// the text past a readable zoom and keeps the focus in sight.

/** The cubic's points from its path: start, two controls, end. */
function points(path: string): [number, number][] {
  const numbers = Array.from(path.matchAll(/-?\d+(\.\d+)?/g), (match) => Number(match[0]));
  return [0, 2, 4, 6].map((at) => [numbers[at] ?? Number.NaN, numbers[at + 1] ?? Number.NaN]);
}

function xAt(path: string, t: number): number {
  const [p0, p1, p2, p3] = points(path).map(([x]) => x);
  const u = 1 - t;
  return u * u * u * (p0 ?? 0) + 3 * u * u * t * (p1 ?? 0) + 3 * u * t * t * (p2 ?? 0) + t * t * t * (p3 ?? 0);
}

describe("a link within one column (the right-hand gap)", () => {
  // A box in column 1, its top handle at the middle of its top side; the target three rows down.
  const left = COLUMN_STEP;
  const middle = left + BOX_WIDTH / 2;
  const arc = arcOver(middle, 0, middle, 3 * ROW_STEP);

  it("bends at least 200 px, so its middle clears the column's boxes", () => {
    expect(ARC_BEND).toBeGreaterThanOrEqual(200);
    const farthest = Math.max(...Array.from({ length: 101 }, (_, at) => xAt(arc.path, at / 100)));
    expect(farthest).toBeGreaterThan(left + BOX_WIDTH);
    expect(farthest).toBeLessThan(left + COLUMN_STEP);
  });

  it("puts its label in the middle of the gap, between the boxes and the next column", () => {
    expect(arc.labelX).toBeCloseTo(left + BOX_WIDTH + COLUMN_GAP / 2, 6);
    expect(arc.labelY).toBeLessThan(3 * ROW_STEP);
  });

  it("leaves a link to an earlier column unbent, above the boxes", () => {
    const back = arcOver(middle, 0, middle - COLUMN_STEP, 0);
    const [, c1, c2] = points(back.path);
    expect([c1?.[0], c2?.[0]]).toEqual([middle, middle - COLUMN_STEP]);
    expect(back.labelY).toBeLessThan(0);
  });
});

describe("a readable fit", () => {
  const grid = (columns: number[]) =>
    columns.flatMap((rows, column) => Array.from({ length: rows }, (_, row) => ({ x: COLUMN_STEP * column, y: ROW_STEP * row })));

  it("centres a small graph, never past 100 %", () => {
    const boxes = grid([1, 2]);
    const viewport = readableViewport(boxes, boxes[0] ?? null, 1200, 700);
    expect(viewport?.zoom).toBe(FIT_MAX_ZOOM);
    const width = COLUMN_STEP + BOX_WIDTH;
    const height = ROW_STEP + BOX_HEIGHT;
    expect(viewport?.x).toBeCloseTo(600 - (width / 2) * FIT_MAX_ZOOM, 6);
    expect(viewport?.y).toBeCloseTo(350 - (height / 2) * FIT_MAX_ZOOM, 6);
  });

  it("fits a graph that fits at a readable zoom whole", () => {
    const boxes = grid([1, 6, 6]);
    const viewport = readableViewport(boxes, boxes[0] ?? null, 900, 600);
    const zoom = Math.min((900 - 2 * FIT_MARGIN) / (2 * COLUMN_STEP + BOX_WIDTH), (600 - 2 * FIT_MARGIN) / (5 * ROW_STEP + BOX_HEIGHT));
    expect(zoom).toBeGreaterThan(FIT_MIN_ZOOM);
    expect(viewport?.zoom).toBeCloseTo(zoom, 6);
  });

  it("large, depth 2: a 25-row column keeps the zoom at 0.6 and the focus at the top left margin", () => {
    // Impact from DOM-GEN-01 at depth 2: the focus, 17 at distance 1, 24 and a more-box at distance 2.
    const boxes = grid([1, 17, 25]);
    const focus = boxes[0] ?? null;
    const viewport = readableViewport(boxes, focus, 1100, 736);
    expect(FIT_MIN_ZOOM).toBeGreaterThanOrEqual(0.6);
    expect(viewport?.zoom).toBe(FIT_MIN_ZOOM);
    // The focus box on screen, whole, at the margin; the width fits, so it is centred across.
    const zoom = viewport?.zoom ?? 0;
    const screenY = (viewport?.y ?? 0) + (focus?.y ?? 0) * zoom;
    const screenX = (viewport?.x ?? 0) + (focus?.x ?? 0) * zoom;
    expect(screenY).toBe(FIT_MARGIN);
    expect(screenX).toBeGreaterThanOrEqual(FIT_MARGIN);
    expect(screenX + BOX_WIDTH * zoom).toBeLessThanOrEqual(1100);
  });

  it("anchors the focus on both sides when the graph overflows both", () => {
    const boxes = grid([1, 24, 24, 24, 24, 24]);
    const viewport = readableViewport(boxes, boxes[0] ?? null, 800, 500);
    expect(viewport).toEqual({ x: FIT_MARGIN, y: FIT_MARGIN, zoom: FIT_MIN_ZOOM });
  });

  it("does nothing on a canvas with no size or no box", () => {
    expect(readableViewport(grid([1]), null, 0, 0)).toBeNull();
    expect(readableViewport([], null, 800, 600)).toBeNull();
  });
});
