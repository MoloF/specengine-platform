import { FIT_MARGIN, FIT_MAX_ZOOM, type FitViewport } from "../graph/geometry";

// The Flow's drawing geometry (docs/features/flow-canvas.md "Layout", docs/features/flow-scenario.md
// D7, D8): pure numbers, no drawing library. The walk runs top to bottom: a forward edge is vertical
// runs joined by S-curves in the gaps between rows, drawn as the horizontal form with its axes
// swapped; an arc is an orthogonal line with rounded corners, through the gaps and along its track
// right of the walk; a loop stays beside its box. Each keeps the points it passes, so "crosses no
// box" can be checked on them.

/**
 * Flow's own fit floor (flow-scenario D8): a box's 14 px title is 12.9 px or more at fit, so a
 * picture too wide to read whole is panned, never shrunk past it. Graph keeps its 0.6.
 */
export const FLOW_FIT_MIN_ZOOM = 0.92;

/** Flow's own pinch floor: the owner may zoom out to half, never further (Graph keeps its 0.1). */
export const FLOW_PINCH_MIN_ZOOM = 0.5;

export interface Point {
  x: number;
  y: number;
}

export interface Rect extends Point {
  width: number;
  height: number;
}

/** One horizontal stretch of a forward edge: inside a column, or the end of a box's port. */
export interface Run {
  from: number;
  to: number;
  y: number;
}

/** The band along an arrow a click takes, in px: wide enough to hit, its stroke unseen (docs/features/writer-studio.md D8). */
export const EDGE_HIT = 12;

/** A box of the flow: its title on up to two lines, its marks, its state under them (docs/features/trace-views.md "Flow badges"). */
export const BOX_HEIGHT = 92;

/** A box's width: four of them and their gutters fit a 1100 px studio's canvas at the fit floor (`docs/canon/spec-flow-layout.md` "Layout"). */
export const BOX_WIDTH = 160;

/** How a corner of an arc rounds; a short stretch rounds less. */
export const ARC_RADIUS = 8;

function n(value: number): string {
  return String(Math.round(value * 10) / 10);
}

/** The runs in order as an SVG path: straight along each, an S-curve from one to the next. */
export function curvePath(runs: readonly Run[]): string {
  const parts: string[] = [];
  let last: Run | null = null;
  for (const run of runs) {
    if (last === null) {
      parts.push(`M${n(run.from)},${n(run.y)}`);
    } else if (last.y === run.y) {
      parts.push(`L${n(run.from)},${n(run.y)}`);
    } else {
      const middle = (last.to + run.from) / 2;
      parts.push(`C${n(middle)},${n(last.y)} ${n(middle)},${n(run.y)} ${n(run.from)},${n(run.y)}`);
    }
    if (run.to !== run.from) {
      parts.push(`L${n(run.to)},${n(run.y)}`);
    }
    last = run;
  }
  return parts.join(" ");
}

/** The points a curve joins, each run's two ends, for checks and the label. */
export function curvePoints(runs: readonly Run[]): Point[] {
  return runs.flatMap((run) => [
    { x: run.from, y: run.y },
    { x: run.to, y: run.y },
  ]);
}

/** Where a label sits on a curve: the middle of the first S-curve, of the only run when there is none. */
export function curveMiddle(runs: readonly Run[]): Point {
  const [first, second] = runs;
  if (first === undefined) {
    return { x: 0, y: 0 };
  }
  if (second === undefined) {
    return { x: (first.from + first.to) / 2, y: first.y };
  }
  return { x: (first.to + second.from) / 2, y: (first.y + second.y) / 2 };
}

/** An orthogonal line as an SVG path, each corner rounded. */
export function roundedPath(points: readonly Point[], radius = ARC_RADIUS): string {
  const parts: string[] = [];
  points.forEach((point, index) => {
    const before = points[index - 1];
    const after = points[index + 1];
    if (before === undefined) {
      parts.push(`M${n(point.x)},${n(point.y)}`);
      return;
    }
    if (after === undefined) {
      parts.push(`L${n(point.x)},${n(point.y)}`);
      return;
    }
    const into = Math.hypot(point.x - before.x, point.y - before.y);
    const out = Math.hypot(after.x - point.x, after.y - point.y);
    const round = Math.min(radius, into / 2, out / 2);
    if (round <= 0) {
      return;
    }
    const start = { x: point.x + ((before.x - point.x) / into) * round, y: point.y + ((before.y - point.y) / into) * round };
    const end = { x: point.x + ((after.x - point.x) / out) * round, y: point.y + ((after.y - point.y) / out) * round };
    parts.push(`L${n(start.x)},${n(start.y)} Q${n(point.x)},${n(point.y)} ${n(end.x)},${n(end.y)}`);
  });
  return parts.join(" ");
}

/** How deep a loop runs under its box, and how far apart its two ends sit. */
export const LOOP_DEPTH = 22;
const LOOP_HALF_WIDTH = 18;

/** A box's edge to itself: down from its underside and back into it, arrow last. */
export function loopPath(box: Rect): { path: string; points: Point[]; middle: Point } {
  const x = box.x + box.width / 2;
  const bottom = box.y + box.height;
  const left = x - LOOP_HALF_WIDTH;
  const right = x + LOOP_HALF_WIDTH;
  const low = bottom + (LOOP_DEPTH * 4) / 3;
  return {
    path: `M${n(left)},${n(bottom)} C${n(left)},${n(low)} ${n(right)},${n(low)} ${n(right)},${n(bottom)}`,
    points: [
      { x: left, y: bottom },
      { x: right, y: bottom },
    ],
    middle: { x, y: bottom + LOOP_DEPTH },
  };
}

/** A point with its axes swapped: the horizontal forms drawn top to bottom. */
export function swapped(point: Point): Point {
  return { x: point.y, y: point.x };
}

/** An SVG path with every coordinate pair's axes swapped: `M1,2 C3,4 ...` becomes `M2,1 C4,3 ...`. */
export function swappedPath(path: string): string {
  return path.replace(/(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)/g, "$2,$1");
}

/** A box's edge to itself, beside it: out of its right side and back into it, arrow last. */
export function sideLoopPath(box: Rect): { path: string; points: Point[]; middle: Point } {
  const loop = loopPath({ x: box.y, y: box.x, width: box.height, height: box.width });
  return { path: swappedPath(loop.path), points: loop.points.map(swapped), middle: swapped(loop.middle) };
}

/** Whether a point lies inside a rectangle, its border excluded: a port sits on the border. */
export function pointInside(point: Point, box: Rect): boolean {
  return point.x > box.x && point.x < box.x + box.width && point.y > box.y && point.y < box.y + box.height;
}

/** Whether an axis-parallel stretch passes through a rectangle's inside. */
export function stretchCrosses(a: Point, b: Point, box: Rect): boolean {
  const left = Math.min(a.x, b.x);
  const right = Math.max(a.x, b.x);
  const top = Math.min(a.y, b.y);
  const bottom = Math.max(a.y, b.y);
  return left < box.x + box.width && right > box.x && top < box.y + box.height && bottom > box.y;
}

/** Whether a rectangle lies whole inside the view at this transform. */
export function inView(box: Rect, view: { x: number; y: number; zoom: number; width: number; height: number }): boolean {
  const left = box.x * view.zoom + view.x;
  const top = box.y * view.zoom + view.y;
  return left >= 0 && top >= 0 && left + box.width * view.zoom <= view.width && top + box.height * view.zoom <= view.height;
}

/** The fit's zoom: the flow fills the width, no larger than 100 %, no smaller than Flow's floor. Height has no say: a tall picture is panned. */
function fitZoom(pictureWidth: number, width: number): number {
  return Math.max(FLOW_FIT_MIN_ZOOM, Math.min(FIT_MAX_ZOOM, (width - 2 * FIT_MARGIN) / pictureWidth));
}

/**
 * The fit: the picture at the zoom the width gives, centred along each side it fits; along one it
 * overflows, its start at the margin, top left. A tall picture stays readable and is panned, never
 * shrunk to the canvas's height. Null on a canvas with no size yet.
 */
export function fitOf(picture: { width: number; height: number }, width: number, height: number): FitViewport | null {
  if (width <= 2 * FIT_MARGIN || height <= 2 * FIT_MARGIN) {
    return null;
  }
  const zoom = fitZoom(picture.width, width);
  const along = (size: number, span: number) => (span * zoom <= size - 2 * FIT_MARGIN ? (size - span * zoom) / 2 : FIT_MARGIN);
  return { x: along(width, picture.width), y: along(height, picture.height), zoom };
}

/** A duration token in milliseconds: 0 under reduced motion, or where no style sheet applies. */
export function durationOf(token: string): number {
  const value = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  const number = Number.parseFloat(value);
  if (!Number.isFinite(number)) {
    return 0;
  }
  return value.endsWith("ms") ? number : value.endsWith("s") ? number * 1000 : number;
}
