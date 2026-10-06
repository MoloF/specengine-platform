import { BOX_HEIGHT, BOX_WIDTH, COLUMN_STEP } from "./layout";

// The canvas's drawing geometry (docs/features/ui-graph.md "Canvas"): pure numbers from the
// layout's fixed grid, no drawing library. A link within one column bends into the gap on its
// right; a fit keeps the boxes' text readable and the focus in sight.

/** The empty band between a column's boxes and the next column's. */
export const COLUMN_GAP = COLUMN_STEP - BOX_WIDTH;

/**
 * How far right of the boxes' middle a same-column arc's control points swing. A cubic's apex
 * reaches 3/4 of it: the middle of the right-hand gap, clear of both columns' boxes.
 */
export const ARC_BEND = ((BOX_WIDTH / 2 + COLUMN_GAP / 2) * 4) / 3;

/** An arc's SVG path and where its label sits (the curve's middle). */
export interface Arc {
  path: string;
  labelX: number;
  labelY: number;
}

/**
 * A top-to-top link (the same or an earlier column) as an arc above the boxes; within one column
 * it swings out through the gap on the right, its label in that gap.
 */
export function arcOver(sx: number, sy: number, tx: number, ty: number): Arc {
  const across = Math.abs(tx - sx);
  const sameColumn = across < 1;
  const bend = sameColumn ? ARC_BEND : 0;
  const lift = 40 + Math.min(160, across * 0.18) + (sameColumn ? Math.abs(ty - sy) * 0.25 : 0);
  const c1x = sx + bend;
  const c1y = sy - lift;
  const c2x = tx + bend;
  const c2y = ty - lift;
  return {
    path: `M${String(sx)},${String(sy)} C${String(c1x)},${String(c1y)} ${String(c2x)},${String(c2y)} ${String(tx)},${String(ty)}`,
    labelX: 0.125 * sx + 0.375 * c1x + 0.375 * c2x + 0.125 * tx,
    labelY: 0.125 * sy + 0.375 * c1y + 0.375 * c2y + 0.125 * ty,
  };
}

/** Pixels kept free around the fitted graph, each side. */
export const FIT_MARGIN = 32;

/** A fit never enlarges past 100 %. */
export const FIT_MAX_ZOOM = 1;

/** A fit never shrinks the boxes' text past this: a bigger graph is panned from its focus. */
export const FIT_MIN_ZOOM = 0.6;

export interface FitViewport {
  x: number;
  y: number;
  zoom: number;
}

/**
 * Where a fit puts the view: the whole graph centred when it fits at a readable zoom; else the
 * zoom stays at `FIT_MIN_ZOOM` and, along each side the graph overflows, the focus box (the
 * walk's start, top left) sits at the margin, its neighbours beside it. Null on a canvas with no
 * size yet.
 */
export function readableViewport(
  boxes: readonly { x: number; y: number }[],
  focus: { x: number; y: number } | null,
  width: number,
  height: number,
): FitViewport | null {
  const roomX = width - 2 * FIT_MARGIN;
  const roomY = height - 2 * FIT_MARGIN;
  if (boxes.length === 0 || roomX <= 0 || roomY <= 0) {
    return null;
  }
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const box of boxes) {
    left = Math.min(left, box.x);
    top = Math.min(top, box.y);
    right = Math.max(right, box.x + BOX_WIDTH);
    bottom = Math.max(bottom, box.y + BOX_HEIGHT);
  }
  const spanX = right - left;
  const spanY = bottom - top;
  const zoom = Math.max(FIT_MIN_ZOOM, Math.min(FIT_MAX_ZOOM, roomX / spanX, roomY / spanY));
  const anchor = focus ?? { x: left, y: top };
  const along = (size: number, room: number, start: number, span: number, at: number) =>
    span * zoom <= room ? size / 2 - (start + span / 2) * zoom : FIT_MARGIN - at * zoom;
  return { x: along(width, roomX, left, spanX, anchor.x), y: along(height, roomY, top, spanY, anchor.y), zoom };
}
