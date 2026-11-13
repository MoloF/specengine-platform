import type { MapPalette } from "./colours";
import type { Kept, Lit, LineStyle } from "./filters";

// The map drawn on one 2D canvas (docs/features/corpus-map.md "Acceptance criteria", CM7, CM12):
// pure over a pen shaped like CanvasRenderingContext2D, so a test records it. Two layers. The base
// is every kept node and line, drawn once into an offscreen canvas and drawn again only when the
// picture, a filter, the theme, the size or the view changes. A frame composes it: the surface, the
// base image (at MAP_DIM_ALPHA while something is lit), then the highlight, which draws only the lit
// nodes, their lines and their labels at full alpha, and the focus ring. Lines are batched by style
// and nodes by colour, one path and one stroke or fill each. A node's disc grows with its uses; an
// orphan gets a dashed ring, a ghost is a hollow dashed circle. Labels go on the MAP_LABELS largest
// nodes and on lit ones, from MAP_LABEL_ZOOM up, over a halo of the surface. Widths, dashes and type
// keep their size on screen at any zoom.

/** The alpha of whatever a lit set leaves out. */
export const MAP_DIM_ALPHA = 0.15;

/** How many of the largest nodes carry a label at rest. */
export const MAP_LABELS = 40;

/** Labels show from this zoom up. */
export const MAP_LABEL_ZOOM = 0.8;

/** The zoom's range. */
export const MAP_ZOOM_MIN = 0.2;
export const MAP_ZOOM_MAX = 4;

/** A ghost's radius, and a node's at most. */
export const GHOST_RADIUS = 3;
export const RADIUS_MAX = 18;

/** A label's length at most, in characters: the card and the List show it whole. */
export const LABEL_CHARS = 32;

/** A node's radius in map units: 3 + 2 sqrt(used_by), at most 18. */
export function radiusOf(usedBy: number): number {
  return Math.min(RADIUS_MAX, 3 + 2 * Math.sqrt(Math.max(0, usedBy)));
}

/** The calls the drawing makes; CanvasRenderingContext2D has each. */
export interface MapPen {
  globalAlpha: number;
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  lineJoin: CanvasLineJoin;
  font: string;
  textAlign: CanvasTextAlign;
  textBaseline: CanvasTextBaseline;
  setTransform: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  fillRect: (x: number, y: number, width: number, height: number) => void;
  beginPath: () => void;
  moveTo: (x: number, y: number) => void;
  lineTo: (x: number, y: number) => void;
  arc: (x: number, y: number, radius: number, start: number, end: number) => void;
  fill: () => void;
  stroke: () => void;
  setLineDash: (segments: number[]) => void;
  fillText: (text: string, x: number, y: number) => void;
  strokeText: (text: string, x: number, y: number) => void;
  drawImage: (image: CanvasImageSource, dx: number, dy: number, dw: number, dh: number) => void;
}

/** Where the picture looks: the map point at the box's middle, and the zoom. */
export interface MapView {
  x: number;
  y: number;
  zoom: number;
}

/** The box in CSS pixels, and the device's pixel ratio. */
export interface MapBox {
  width: number;
  height: number;
  ratio: number;
}

/** What a drawing draws: each point's place, disc, colour slot, marks and words; each line's ends and look. */
export interface MapPicture {
  positions: Float64Array;
  radii: Float64Array;
  /** The colour slot of a node, 0 the grey. */
  slots: Uint8Array;
  ghost: Uint8Array;
  orphan: Uint8Array;
  labels: readonly string[];
  /** The MAP_LABELS largest drawn nodes, 1 each. */
  labelled: Uint8Array;
  lineFrom: Int32Array;
  lineTo: Int32Array;
  lineStyle: readonly LineStyle[];
}

/** What a drawing did, counted: what a performance bound reads (CM12). */
export interface DrawCount {
  nodes: number;
  lines: number;
  fills: number;
  strokes: number;
  labels: number;
}

/** A screen point's place on the map. */
export function toMap(view: MapView, box: Pick<MapBox, "width" | "height">, sx: number, sy: number): { x: number; y: number } {
  return { x: view.x + (sx - box.width / 2) / view.zoom, y: view.y + (sy - box.height / 2) / view.zoom };
}

/** A zoom kept in range. */
export function clampZoom(zoom: number): number {
  return Math.min(MAP_ZOOM_MAX, Math.max(MAP_ZOOM_MIN, zoom));
}

/** The view that shows every kept point with a margin; the middle at zoom 1 when none is kept. */
export function fitView(positions: Float64Array, radii: Float64Array, kept: Uint8Array, box: Pick<MapBox, "width" | "height">, margin = 24): MapView {
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (let point = 0; point < kept.length; point += 1) {
    if (kept[point] !== 1) {
      continue;
    }
    const x = positions[point * 2] ?? 0;
    const y = positions[point * 2 + 1] ?? 0;
    const r = radii[point] ?? 0;
    left = Math.min(left, x - r);
    right = Math.max(right, x + r);
    top = Math.min(top, y - r);
    bottom = Math.max(bottom, y + r);
  }
  if (left === Infinity) {
    return { x: 0, y: 0, zoom: 1 };
  }
  const zoom = clampZoom(Math.min((box.width - 2 * margin) / Math.max(1, right - left), (box.height - 2 * margin) / Math.max(1, bottom - top)));
  return { x: (left + right) / 2, y: (top + bottom) / 2, zoom };
}

/** The MAP_LABELS largest of the drawn nodes, ties in scene order. */
export function labelledOf(radii: Float64Array, ghost: Uint8Array): Uint8Array {
  const order = Array.from(radii.keys())
    .filter((point) => ghost[point] !== 1)
    .sort((a, b) => (radii[b] ?? 0) - (radii[a] ?? 0) || a - b)
    .slice(0, MAP_LABELS);
  const labelled = new Uint8Array(radii.length);
  for (const point of order) {
    labelled[point] = 1;
  }
  return labelled;
}

/** A label cut to LABEL_CHARS characters, whole when shorter. */
export function labelOf(text: string): string {
  const chars = Array.from(text);
  return chars.length <= LABEL_CHARS ? text : `${chars.slice(0, LABEL_CHARS - 1).join("")}\u2026`;
}

/** Line looks: colour, width and dash, in screen pixels. */
function lineLook(style: LineStyle, palette: MapPalette): { colour: string; width: number; dash: number[] } {
  switch (style) {
    case "strong":
      return { colour: palette.edge, width: 1.25, dash: [] };
    case "weak":
      return { colour: palette.weak, width: 0.75, dash: [4, 3] };
    case "parent":
      return { colour: palette.edge, width: 1, dash: [1, 3] };
    case "ghost":
      return { colour: palette.ghost, width: 1, dash: [3, 3] };
  }
}

const LINE_STYLES: readonly LineStyle[] = ["parent", "weak", "ghost", "strong"];

/** The world's transform for a view: map units to device pixels. */
function toWorld(pen: MapPen, view: MapView, box: MapBox): void {
  pen.setTransform(box.ratio * view.zoom, 0, 0, box.ratio * view.zoom, box.ratio * (box.width / 2 - view.x * view.zoom), box.ratio * (box.height / 2 - view.y * view.zoom));
}

/**
 * Draws the given points and lines, each once, at the pen's alpha: the lines one path per look, the
 * discs one path per colour slot, the orphans' rings and the ghosts one path each, then the labels
 * of the points `labelled` names. `wide` draws the lines 1.6 times as wide, as a highlight does.
 */
function drawLayer(
  pen: MapPen,
  picture: MapPicture,
  points: readonly number[],
  lines: readonly number[],
  labelled: (point: number) => boolean,
  view: MapView,
  palette: MapPalette,
  wide: boolean,
  count: DrawCount,
): void {
  const { positions, radii } = picture;
  const px = (pixels: number) => pixels / view.zoom;
  for (const style of LINE_STYLES) {
    let any = false;
    pen.beginPath();
    for (const line of lines) {
      if (picture.lineStyle[line] !== style) {
        continue;
      }
      const a = picture.lineFrom[line] ?? 0;
      const b = picture.lineTo[line] ?? 0;
      pen.moveTo(positions[a * 2] ?? 0, positions[a * 2 + 1] ?? 0);
      pen.lineTo(positions[b * 2] ?? 0, positions[b * 2 + 1] ?? 0);
      count.lines += 1;
      any = true;
    }
    if (any) {
      const look = lineLook(style, palette);
      pen.strokeStyle = look.colour;
      pen.lineWidth = px(look.width * (wide ? 1.6 : 1));
      pen.setLineDash(look.dash.map(px));
      pen.stroke();
      count.strokes += 1;
    }
  }
  pen.setLineDash([]);
  for (let slot = 0; slot < palette.slots.length; slot += 1) {
    let any = false;
    pen.beginPath();
    for (const point of points) {
      if (picture.ghost[point] === 1 || picture.slots[point] !== slot) {
        continue;
      }
      const x = positions[point * 2] ?? 0;
      const y = positions[point * 2 + 1] ?? 0;
      const r = radii[point] ?? 0;
      pen.moveTo(x + r, y);
      pen.arc(x, y, r, 0, Math.PI * 2);
      count.nodes += 1;
      any = true;
    }
    if (any) {
      pen.fillStyle = palette.slots[slot] ?? palette.slots[0] ?? palette.edge;
      pen.fill();
      count.fills += 1;
    }
  }
  for (const mark of ["orphan", "ghost"] as const) {
    let any = false;
    pen.beginPath();
    for (const point of points) {
      const marked = mark === "orphan" ? picture.orphan[point] === 1 && picture.ghost[point] !== 1 : picture.ghost[point] === 1;
      if (!marked) {
        continue;
      }
      const x = positions[point * 2] ?? 0;
      const y = positions[point * 2 + 1] ?? 0;
      const r = mark === "orphan" ? (radii[point] ?? 0) + px(3) : (radii[point] ?? GHOST_RADIUS);
      pen.moveTo(x + r, y);
      pen.arc(x, y, r, 0, Math.PI * 2);
      if (mark === "ghost") {
        count.nodes += 1;
      }
      any = true;
    }
    if (any) {
      pen.strokeStyle = mark === "orphan" ? palette.label : palette.ghost;
      pen.lineWidth = px(mark === "orphan" ? 1.25 : 1.5);
      pen.setLineDash([px(2), px(2)]);
      pen.stroke();
      count.strokes += 1;
    }
  }
  pen.setLineDash([]);
  if (view.zoom >= MAP_LABEL_ZOOM) {
    pen.font = `${String(px(12))}px ${palette.font}`;
    pen.textAlign = "center";
    pen.textBaseline = "top";
    pen.lineJoin = "round";
    pen.lineWidth = px(3);
    for (const point of points) {
      if (picture.ghost[point] === 1 || !labelled(point)) {
        continue;
      }
      const text = picture.labels[point] ?? "";
      const x = positions[point * 2] ?? 0;
      const y = (positions[point * 2 + 1] ?? 0) + (radii[point] ?? 0) + px(3);
      pen.strokeStyle = palette.surface;
      pen.strokeText(text, x, y);
      pen.fillStyle = palette.label;
      pen.fillText(text, x, y);
      count.labels += 1;
    }
  }
}

/** A fresh count. */
function noCount(): DrawCount {
  return { nodes: 0, lines: 0, fills: 0, strokes: 0, labels: 0 };
}

/** The indexes a flag array sets. */
function setIn(flags: Uint8Array): number[] {
  const out: number[] = [];
  for (let at = 0; at < flags.length; at += 1) {
    if (flags[at] === 1) {
      out.push(at);
    }
  }
  return out;
}

/**
 * The base layer: the surface, then every kept line and node once at full alpha, the marks and the
 * labels of the MAP_LABELS largest. Drawn into the offscreen canvas; a hover never draws it.
 */
export function drawBase(pen: MapPen, picture: MapPicture, kept: Kept, view: MapView, box: MapBox, palette: MapPalette): DrawCount {
  const count = noCount();
  pen.setTransform(box.ratio, 0, 0, box.ratio, 0, 0);
  pen.globalAlpha = 1;
  pen.fillStyle = palette.surface;
  pen.fillRect(0, 0, box.width, box.height);
  toWorld(pen, view, box);
  drawLayer(pen, picture, setIn(kept.points), setIn(kept.lines), (point) => picture.labelled[point] === 1, view, palette, false, count);
  return count;
}

/** The highlight: the lit points and lines the filters keep, and only them, at full alpha, each lit point labelled. */
export function drawHighlight(pen: MapPen, picture: MapPicture, kept: Kept, lit: Lit, view: MapView, box: MapBox, palette: MapPalette): DrawCount {
  const count = noCount();
  pen.globalAlpha = 1;
  toWorld(pen, view, box);
  const points = lit.pointList.filter((point) => kept.points[point] === 1);
  const lines = lit.lineList.filter((line) => kept.lines[line] === 1);
  drawLayer(pen, picture, points, lines, () => true, view, palette, true, count);
  return count;
}

/** The base as drawn: its image, and the view and box it was drawn for. */
export interface MapBase {
  image: CanvasImageSource;
  view: MapView;
  box: MapBox;
}

/**
 * One frame: the surface, the base image moved and scaled from its view to this one (at
 * MAP_DIM_ALPHA while something is lit), then the highlight and the focus ring. A pan or a zoom
 * draws the base where it now falls until it is drawn again for the new view.
 */
export function composeMap(pen: MapPen, base: MapBase, picture: MapPicture, kept: Kept, lit: Lit | null, focus: number | null, view: MapView, box: MapBox, palette: MapPalette): DrawCount {
  pen.setTransform(box.ratio, 0, 0, box.ratio, 0, 0);
  pen.globalAlpha = 1;
  pen.fillStyle = palette.surface;
  pen.fillRect(0, 0, box.width, box.height);
  const scale = view.zoom / base.view.zoom;
  // A map point the base drew at p0 = (x - x0) * z0 + w0 / 2 falls now at (x - x1) * z1 + w1 / 2 = p0 * s + dx.
  const dx = box.width / 2 - (base.box.width / 2) * scale + (base.view.x - view.x) * view.zoom;
  const dy = box.height / 2 - (base.box.height / 2) * scale + (base.view.y - view.y) * view.zoom;
  pen.globalAlpha = lit === null ? 1 : MAP_DIM_ALPHA;
  pen.drawImage(base.image, dx, dy, base.box.width * scale, base.box.height * scale);
  const count = lit === null ? noCount() : drawHighlight(pen, picture, kept, lit, view, box, palette);
  pen.globalAlpha = 1;
  if (focus !== null && focus >= 0 && focus < picture.radii.length && kept.points[focus] === 1) {
    toWorld(pen, view, box);
    pen.beginPath();
    const x = picture.positions[focus * 2] ?? 0;
    const y = picture.positions[focus * 2 + 1] ?? 0;
    const r = (picture.radii[focus] ?? 0) + 5 / view.zoom;
    pen.moveTo(x + r, y);
    pen.arc(x, y, r, 0, Math.PI * 2);
    pen.strokeStyle = palette.focus;
    pen.lineWidth = 3 / view.zoom;
    pen.stroke();
    count.strokes += 1;
  }
  return count;
}
