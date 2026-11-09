import type { FlowEdge, FlowNode, FlowView } from "../api/types";
import { BOX_WIDTH, CANVAS_LIMIT, compareText, nodeName } from "../graph/layout";
import { BOX_HEIGHT, curveMiddle, curvePath, curvePoints, loopPath, LOOP_DEPTH, roundedPath, type Point, type Rect, type Run } from "./geometry";
import { edgeWords } from "./labels";

// The Flow's layout (docs/features/flow-canvas.md "Layout"), hand-written and pure: one read in
// (docs/features/writer-studio.md D1: a group is entered, never opened in place), boxes, its lane and
// edges at absolute positions out. A box's column is its `distance` in the read, rows come from four
// barycentre sweeps. Columns decide every edge's shape, never a type or a `kind`, so a loop nobody
// declared shows as well as one declared. Nothing here follows, lifts or counts a link: each edge
// drawn is the read's, unchanged. The input is sorted first, so the same read in any order gives the
// same picture.

export { BOX_HEIGHT, BOX_WIDTH, CANVAS_LIMIT };

/** The empty band between two columns, wide enough for a two-line edge label. */
export const COLUMN_GAP = 136;
/** Between two boxes of a column. */
export const ROW_GAP = 24;
/** Inside a lane, each side. */
export const LANE_PAD = 20;
/** From one arc's track to the next. */
export const TRACK_STEP = 16;
/** From one vertical stretch to the next in a gap. */
export const SLOT_STEP = 10;
/** From one port to the next on a box's side. */
export const PORT_STEP = 8;

/** What a long forward edge reserves in a column it passes, and its distance to what is beside it. */
const PASSAGE_HEIGHT = 2;
const PASSAGE_GAP = 12;
/** Kept free under a box with an edge to itself: the loop and its label, under the loop. */
const LOOP_LABEL_DROP = 16;
const LOOP_ROOM = LOOP_DEPTH + LOOP_LABEL_DROP + 20;
/** Over the arc band's first track, and from a gap's side to its first vertical stretch. */
const ARC_PAD = 18;
const GAP_INSET = 12;
/** The band's caption line, the space between its boxes, its columns at least. */
const BAND_HEAD = 30;
const BAND_GAP = 44;
const BAND_ROW_GAP = 20;
const BAND_MIN_COLUMNS = 4;
/** Passes that align each column with its neighbours once the rows are ordered. */
const PLACE_PASSES = 8;
/** An edge label's width at most, its line height, and a character's mean width, for telling two apart. */
const LABEL_WIDTH = 128;
const LABEL_LINE = 14;
const LABEL_CHARACTER = 6;
/** Above every real row: an arc's port sorts under the forward ones, a deeper track above a shallower. */
const BELOW_THE_ROWS = 1_000_000;

export interface FlowBox extends Rect {
  /** `n:` and the name, the same box in every layout; a second holder of the name adds its path and line. */
  id: string;
  name: string;
  node: FlowNode;
  /** Its column in the lane; null in the band under it. */
  column: number | null;
  row: number;
}

export interface FlowLane extends Rect {
  /** Each column's box ids, top to bottom. */
  columns: string[][];
  /** The band's rows of box ids, left to right. */
  band: string[][];
  /** Where the band's caption sits; null with no band. */
  bandY: number | null;
}

/** Forward, against the walk, into the gap beside, to itself, or to or from the band. */
export type EdgeRoute = "forward" | "arc" | "beside" | "self" | "band";

export interface LaidEdge {
  id: string;
  /** The read's edge, unchanged. */
  edge: FlowEdge;
  source: string;
  target: string;
  route: EdgeRoute;
  /** The corners of an arc, the ends of a curve's runs. */
  points: Point[];
  path: string;
  label: Point;
}

export interface FlowLayout {
  /** Drawn boxes, a head before its lane. */
  boxes: FlowBox[];
  lanes: FlowLane[];
  edges: LaidEdge[];
  width: number;
  height: number;
  /** Boxes the read holds, drawn or not. */
  total: number;
  notDrawn: number;
}

/** A box's canvas id; a second holder of one ID adds its place. */
export function boxId(name: string): string {
  return `n:${name}`;
}

/** What tells two holders of one ID apart. */
function placeOf(node: FlowNode): string {
  return `${node.path}:${String(node.line)}`;
}

/** The daemon's order: by distance, unreached last, then path. */
function byServed(a: FlowNode, b: FlowNode): number {
  const near = a.distance ?? Infinity;
  const far = b.distance ?? Infinity;
  return (near === far ? 0 : near < far ? -1 : 1) || compareText(a.path, b.path) || compareText(nodeName(a), nodeName(b));
}

/** A total order of a read's edges. */
function byEdge(a: FlowEdge, b: FlowEdge): number {
  return (
    compareText(a.from, b.from) ||
    compareText(a.to, b.to) ||
    compareText(a.type, b.type) ||
    compareText(a.kind ?? "", b.kind ?? "") ||
    compareText(a.when ?? "", b.when ?? "") ||
    compareText(a.path, b.path) ||
    a.line - b.line
  );
}

/** Whether the box is a group, entered as a scope: it holds documents. */
export function isGroup(node: FlowNode): boolean {
  return node.holds > 0;
}

/**
 * The cap: the first `CANVAS_LIMIT` places of the read in served order are drawn; past it boxes stay
 * undrawn, with their edges, and the List holds them all (docs/features/writer-studio.md R6).
 */
function drawnOf(view: FlowView): { drawn: FlowNode[]; total: number } {
  const boxes = [...view.nodes].sort(byServed);
  const placed = new Set<string>();
  const drawn: FlowNode[] = [];
  for (const node of boxes) {
    if (placed.size >= CANVAS_LIMIT) {
      break;
    }
    if (!placed.has(placeOf(node))) {
      placed.add(placeOf(node));
      drawn.push(node);
    }
  }
  return { drawn, total: boxes.length };
}

/** A box in its lane while the lane is laid out, positions relative to the lane. */
interface Cell {
  id: string;
  name: string;
  node: FlowNode;
  width: number;
  height: number;
  /** From its top to its ports' middle. */
  anchor: number;
  loops: boolean;
  column: number | null;
  row: number;
  x: number;
  y: number;
  /** Its band column, for a box in the band. */
  bandColumn: number;
}

/** A place in a column: a box, or the passage of a long forward edge. */
interface Slot {
  cell: Cell | null;
  height: number;
  anchor: number;
  rank: number;
  row: number;
  y: number;
  before: Slot[];
  after: Slot[];
}

interface LaneEdge {
  edge: FlowEdge;
  id: string;
  from: Cell;
  to: Cell;
  route: EdgeRoute;
  /** Its slots, source to target (a forward edge). */
  chain: Slot[];
  /** Its place in the arc band; -1 for none. */
  track: number;
  /** Two band boxes side by side: a line from port to port, no track. */
  direct: boolean;
  /** Who shares its stretch out of the source, into the target. */
  outKey: string;
  inKey: string;
  outChannel: Channel | null;
  inChannel: Channel | null;
}

/** A gap: left of a column, or the margin past the last. */
interface Channel {
  runs: Map<string, { track: number; into: boolean }>;
  x: number;
  width: number;
  slots: Map<string, number>;
}

/** The part of an edge that survives placing a lane inside another. */
type Shape = { runs: Run[] } | { points: Point[] } | { box: Cell };

interface Draft {
  cells: Cell[];
  edges: { laid: LaneEdge; shape: Shape; label: Point }[];
  columns: Cell[][];
  band: Cell[][];
  bandY: number | null;
  width: number;
  height: number;
}

/** A barycentre as a fraction, compared without rounding. */
interface Key {
  sum: number;
  count: number;
}

function compareKeys(a: Key, b: Key): number {
  return a.sum * b.count - b.sum * a.count;
}

/** Reorders a column by the mean row of each slot's neighbours; a slot with none keeps its row. */
function sweepColumn(slots: readonly Slot[], near: (slot: Slot) => readonly Slot[]): Slot[] {
  const keys = new Map<Slot, Key>();
  for (const slot of slots) {
    const others = near(slot);
    keys.set(slot, others.length === 0 ? { sum: slot.row, count: 1 } : { sum: others.reduce((sum, other) => sum + other.row, 0), count: others.length });
  }
  const sorted = [...slots].sort((a, b) => compareKeys(keys.get(a) ?? { sum: 0, count: 1 }, keys.get(b) ?? { sum: 0, count: 1 }) || a.rank - b.rank);
  sorted.forEach((slot, row) => {
    slot.row = row;
  });
  return sorted;
}

/**
 * Tops as near as can be to `wanted`, the order kept, `gaps[i]` between items i and i + 1: subtracting
 * each item's least offset turns the spacing into "never decreasing", which pooling adjacent
 * violators solves exactly.
 */
function settle(wanted: readonly number[], sizes: readonly number[], gaps: readonly number[]): number[] {
  const offsets: number[] = [];
  let offset = 0;
  sizes.forEach((size, index) => {
    offsets.push(offset);
    offset += size + (gaps[index] ?? 0);
  });
  const pools: { sum: number; count: number }[] = [];
  wanted.forEach((value, index) => {
    pools.push({ sum: value - (offsets[index] ?? 0), count: 1 });
    for (let last = pools[pools.length - 1], before = pools[pools.length - 2]; last !== undefined && before !== undefined && before.sum / before.count > last.sum / last.count; last = pools[pools.length - 1], before = pools[pools.length - 2]) {
      before.sum += last.sum;
      before.count += last.count;
      pools.pop();
    }
  });
  const tops: number[] = [];
  for (const pool of pools) {
    for (let index = 0; index < pool.count; index += 1) {
      tops.push(pool.sum / pool.count + (offsets[tops.length] ?? 0));
    }
  }
  return tops;
}

function middleOf(slot: Slot): number {
  return slot.y + slot.anchor;
}

/** Aligns one column with its neighbours on `near`; a slot with none stays where it is. */
function placeColumn(slots: readonly Slot[], near: (slot: Slot) => readonly Slot[]): void {
  const wanted = slots.map((slot) => {
    const others = near(slot);
    return others.length === 0 ? slot.y : others.reduce((sum, other) => sum + middleOf(other), 0) / others.length - slot.anchor;
  });
  const gaps = slots.map((slot, index) => {
    const next = slots[index + 1];
    return slot.cell === null || next?.cell === null ? PASSAGE_GAP : ROW_GAP;
  });
  const tops = settle(
    wanted,
    slots.map((slot) => slot.height),
    gaps,
  );
  slots.forEach((slot, index) => {
    slot.y = tops[index] ?? slot.y;
  });
}

function emptyChannel(): Channel {
  return { runs: new Map(), x: 0, width: 0, slots: new Map() };
}

/** Asks a channel for a stretch; a key already there shares it. */
function ask(channel: Channel, key: string, track: number, into: boolean): void {
  if (!channel.runs.has(key)) {
    channel.runs.set(key, { track, into });
  }
}

/**
 * Orders a channel's stretches so none crosses its neighbour's corner: those into a box first, the
 * deepest track furthest left; then those out of one, the deepest furthest right.
 */
function slotsOf(channel: Channel): void {
  const entries = [...channel.runs.entries()];
  const into = entries.filter(([, run]) => run.into).sort(([ka, a], [kb, b]) => b.track - a.track || compareText(ka, kb));
  const out = entries.filter(([, run]) => !run.into).sort(([ka, a], [kb, b]) => a.track - b.track || compareText(ka, kb));
  [...into, ...out].forEach(([key], index) => {
    channel.slots.set(key, index);
  });
}

function slotX(channel: Channel | null, key: string): number {
  if (channel === null) {
    return 0;
  }
  return channel.x + GAP_INSET + (channel.slots.get(key) ?? 0) * SLOT_STEP;
}

/** How wide a gap has to be for its stretches. */
function channelWidth(channel: Channel, least: number): number {
  return Math.max(least, GAP_INSET * 2 + channel.runs.size * SLOT_STEP);
}

/** One side of each box: who ends there, in what order, at what offset from its port's middle. */
class Sides {
  private readonly uses = new Map<Cell, Map<string, number>>();

  add(cell: Cell, key: string, order: number): void {
    const side = this.uses.get(cell) ?? new Map<string, number>();
    if (!side.has(key)) {
      side.set(key, order);
    }
    this.uses.set(cell, side);
  }

  /** The port's y, relative to the lane. */
  y(cell: Cell, key: string): number {
    const side = this.uses.get(cell);
    const middle = cell.y + cell.anchor;
    if (side === undefined || side.size < 2) {
      return middle;
    }
    const ordered = [...side.entries()].sort(([ka, a], [kb, b]) => a - b || compareText(ka, kb)).map(([name]) => name);
    const room = (cell.anchor * 2 - 16) / (ordered.length - 1);
    const step = Math.min(PORT_STEP, room);
    return middle + (ordered.indexOf(key) - (ordered.length - 1) / 2) * step;
  }
}

function slotOf(cell: Cell, rank: number): Slot {
  return {
    cell,
    height: cell.height + (cell.loops ? LOOP_ROOM : 0),
    anchor: cell.anchor,
    rank,
    row: rank,
    y: 0,
    before: [],
    after: [],
  };
}

/**
 * The band's order: under the columns each box links to, those linked to none last; a box followed
 * at once by what it leads to, so a chain off the walk reads as one, left to right.
 */
function bandOrderOf(cells: readonly Cell[], edges: readonly LaneEdge[]): Cell[] {
  const places = new Map<Cell, number[]>();
  const leads = new Map<Cell, Cell[]>();
  const led = new Set<Cell>();
  for (const { from, to } of edges) {
    if (from.column === null && to.column !== null) {
      places.set(from, [...(places.get(from) ?? []), to.column]);
    } else if (to.column === null && from.column !== null) {
      places.set(to, [...(places.get(to) ?? []), from.column]);
    } else if (from.column === null && to.column === null && from !== to) {
      leads.set(from, [...(leads.get(from) ?? []), to]);
      led.add(to);
    }
  }
  const reach = (cell: Cell): number => {
    const seen = places.get(cell) ?? [];
    return seen.length === 0 ? Infinity : seen.reduce((sum, place) => sum + place, 0) / seen.length;
  };
  const heads = cells
    .map((cell, index) => ({ cell, index, near: reach(cell) }))
    .sort((a, b) => Number(led.has(a.cell)) - Number(led.has(b.cell)) || (a.near === b.near ? 0 : a.near < b.near ? -1 : 1) || a.index - b.index)
    .map(({ cell }) => cell);
  const ordered: Cell[] = [];
  const placed = new Set<Cell>();
  const take = (cell: Cell) => {
    const stack = [cell];
    for (let at = stack.pop(); at !== undefined; at = stack.pop()) {
      if (!placed.has(at)) {
        placed.add(at);
        ordered.push(at);
        stack.push(...[...(leads.get(at) ?? [])].reverse());
      }
    }
  };
  heads.forEach(take);
  return ordered;
}

/** Whether two band boxes stand side by side, the first on the left: nothing lies between their ports. */
function sideBySide(from: Cell, to: Cell): boolean {
  return from.column === null && to.column === null && from.row === to.row && to.bandColumn === from.bandColumn + 1;
}

function routeOf(from: Cell, to: Cell): EdgeRoute {
  if (from === to) {
    return "self";
  }
  if (from.column === null || to.column === null) {
    return "band";
  }
  if (to.column < from.column) {
    return "arc";
  }
  return to.column === from.column ? "beside" : "forward";
}

function edgeKey(edge: FlowEdge): string {
  return `e:${edge.from}>${edge.to}:${edge.type}:${edge.kind ?? ""}`;
}

function linked(a: Slot, b: Slot): void {
  a.after.push(b);
  b.before.push(a);
}

/** The lane's boxes, the cap's alone; a name's first holder takes its plain id. */
function cellsOf(drawn: readonly FlowNode[]): Cell[] {
  const held = new Set<string>();
  return drawn.map((node): Cell => {
    const name = nodeName(node);
    const first = !held.has(name);
    held.add(name);
    return {
      id: first ? boxId(name) : `${boxId(name)}@${placeOf(node)}`,
      name,
      node,
      width: BOX_WIDTH,
      height: BOX_HEIGHT,
      anchor: BOX_HEIGHT / 2,
      loops: false,
      column: null,
      row: 0,
      x: 0,
      y: 0,
      bandColumn: 0,
    };
  });
}

/** Lays out the read's lane, relative to its top left. */
function draftLane(view: FlowView, drawn: readonly FlowNode[]): Draft {
  const cells = cellsOf(drawn);
  // A link lands on a name's first holder.
  const byName = new Map([...cells].reverse().map((cell) => [cell.name, cell]));
  const layers = [...new Set(cells.flatMap((cell) => (cell.node.distance === null ? [] : [cell.node.distance])))].sort((a, b) => a - b);
  const columnOf = new Map(layers.map((layer, column) => [layer, column]));
  for (const cell of cells) {
    cell.column = cell.node.distance === null ? null : (columnOf.get(cell.node.distance) ?? null);
  }

  const edges: LaneEdge[] = [...view.edges].sort(byEdge).flatMap((edge) => {
    const from = byName.get(edge.from);
    const to = byName.get(edge.to);
    if (from === undefined || to === undefined) {
      return [];
    }
    const id = edgeKey(edge);
    return [{ edge, id, from, to, route: routeOf(from, to), chain: [], track: -1, direct: false, outKey: id, inKey: id, outChannel: null, inChannel: null }];
  });
  for (const laid of edges) {
    if (laid.route === "self") {
      laid.from.loops = true;
    }
  }

  // Columns, a passage for each column a forward edge skips; four sweeps, as layoutGraph.
  const columns: Slot[][] = layers.map(() => []);
  const slotByCell = new Map<Cell, Slot>();
  let rank = 0;
  for (const cell of cells) {
    if (cell.column !== null) {
      const slot = slotOf(cell, rank);
      rank += 1;
      slotByCell.set(cell, slot);
      columns[cell.column]?.push(slot);
    }
  }
  for (const laid of edges) {
    const source = slotByCell.get(laid.from);
    const target = slotByCell.get(laid.to);
    if (laid.route !== "forward" || source === undefined || target === undefined || laid.from.column === null || laid.to.column === null) {
      continue;
    }
    const chain = [source];
    for (let column = laid.from.column + 1; column < laid.to.column; column += 1) {
      const passage: Slot = { cell: null, height: PASSAGE_HEIGHT, anchor: PASSAGE_HEIGHT / 2, rank, row: rank, y: 0, before: [], after: [] };
      rank += 1;
      columns[column]?.push(passage);
      chain.push(passage);
    }
    chain.push(target);
    chain.forEach((slot, index) => {
      const next = chain[index + 1];
      if (next !== undefined) {
        linked(slot, next);
      }
    });
    laid.chain = chain;
  }
  for (const slots of columns) {
    slots.forEach((slot, row) => {
      slot.row = row;
    });
  }
  const orders = columns.map((slots) => slots);
  const sweep = (down: boolean) => {
    const sequence = orders.map((_, index) => (down ? index : orders.length - 1 - index)).slice(1);
    for (const at of sequence) {
      orders[at] = sweepColumn(orders[at] ?? [], (slot) => (down ? slot.before : slot.after));
    }
  };
  sweep(true);
  sweep(false);
  sweep(true);
  sweep(false);

  // Each column stacked, then pulled towards its neighbours: a join sits between its branches.
  for (const slots of orders) {
    let top = 0;
    slots.forEach((slot, index) => {
      slot.y = top;
      const next = slots[index + 1];
      top += slot.height + (slot.cell === null || next?.cell === null ? PASSAGE_GAP : ROW_GAP);
    });
  }
  for (let pass = 0; pass < PLACE_PASSES; pass += 1) {
    const down = pass % 2 === 0;
    const sequence = orders.map((_, index) => (down ? index : orders.length - 1 - index));
    for (const at of sequence) {
      placeColumn(orders[at] ?? [], (slot) => (down ? slot.before : slot.after));
    }
  }
  const lowest = Math.min(0, ...orders.flatMap((slots) => slots.map((slot) => slot.y)));
  let gridBottom = LANE_PAD;
  for (const slots of orders) {
    let row = 0;
    for (const slot of slots) {
      slot.y = Math.round(slot.y - lowest) + LANE_PAD;
      gridBottom = Math.max(gridBottom, slot.y + slot.height);
      if (slot.cell !== null) {
        slot.cell.y = slot.y;
        slot.cell.row = row;
        row += 1;
      }
    }
  }

  // The band: its boxes under the columns they link to, in a grid whose gutters stay free.
  const inBand = cells.filter((cell) => cell.column === null);
  const bandColumns = Math.max(1, Math.min(inBand.length, Math.max(layers.length, BAND_MIN_COLUMNS)));
  const bandOrder = bandOrderOf(inBand, edges);
  const band: Cell[][] = [];
  bandOrder.forEach((cell, index) => {
    const row = Math.floor(index / bandColumns);
    cell.row = row;
    cell.bandColumn = index % bandColumns;
    const shelf = band[row] ?? [];
    shelf.push(cell);
    band[row] = shelf;
  });

  // Two band boxes side by side need no track: the line runs from one port to the next.
  for (const laid of edges) {
    if (laid.route === "band" && sideBySide(laid.from, laid.to)) {
      laid.direct = true;
    }
  }

  // Tracks: the shortest arcs nearest the lane, so longer ones wrap them; trunks to the band below.
  const place = (cell: Cell) => cell.column ?? cell.bandColumn;
  const span = (laid: LaneEdge) => Math.abs(place(laid.from) - place(laid.to));
  const arcs = edges.filter((laid) => laid.route === "arc").sort((a, b) => span(a) - span(b) || compareText(a.id, b.id));
  let tracks = 0;
  for (const laid of arcs) {
    laid.track = tracks;
    tracks += 1;
  }
  const trunks = new Map<string, number>();
  const toBand = edges.filter((laid) => laid.route === "band" && !laid.direct).sort((a, b) => span(a) - span(b) || compareText(a.id, b.id));
  for (const laid of toBand) {
    const key = `${laid.to.column === null ? `>${laid.to.name}` : `<${laid.from.name}`}:${laid.edge.type}:${laid.edge.kind ?? ""}`;
    if (laid.to.column === null) {
      laid.inKey = key;
    } else {
      laid.outKey = key;
    }
    const known = trunks.get(key);
    if (known === undefined) {
      trunks.set(key, tracks);
      laid.track = tracks;
      tracks += 1;
    } else {
      laid.track = known;
    }
  }

  // Gaps and gutters: each stretch its own place, a gap widening when it has to.
  const gaps: Channel[] = Array.from({ length: layers.length + 1 }, emptyChannel);
  const gutters: Channel[] = Array.from({ length: bandColumns + 1 }, emptyChannel);
  const outOf = (cell: Cell) => (cell.column === null ? gutters[cell.bandColumn + 1] : gaps[cell.column + 1]) ?? null;
  const intoOf = (cell: Cell) => (cell.column === null ? gutters[cell.bandColumn] : gaps[cell.column]) ?? null;
  for (const laid of edges) {
    if (laid.direct) {
      continue;
    }
    if (laid.route === "arc" || laid.route === "band") {
      laid.outChannel = outOf(laid.from);
      laid.inChannel = intoOf(laid.to);
      if (laid.outChannel !== null) {
        ask(laid.outChannel, laid.outKey, laid.track, false);
      }
      if (laid.inChannel !== null) {
        ask(laid.inChannel, laid.inKey, laid.track, true);
      }
    } else if (laid.route === "beside") {
      laid.outChannel = outOf(laid.from);
      if (laid.outChannel !== null) {
        ask(laid.outChannel, laid.id, -1, false);
      }
    }
  }
  gaps.forEach(slotsOf);
  gutters.forEach(slotsOf);

  let right = 0;
  let x = 0;
  orders.forEach((slots, at) => {
    const gap = gaps[at] ?? emptyChannel();
    gap.width = at === 0 ? channelWidth(gap, LANE_PAD) : channelWidth(gap, COLUMN_GAP);
    gap.x = x;
    x += gap.width;
    const width = Math.max(BOX_WIDTH, ...slots.map((slot) => slot.cell?.width ?? 0));
    for (const slot of slots) {
      if (slot.cell !== null) {
        slot.cell.x = x;
      }
    }
    x += width;
    right = x;
  });
  const lastGap = gaps[orders.length];
  if (lastGap !== undefined && orders.length > 0) {
    lastGap.width = channelWidth(lastGap, LANE_PAD);
    lastGap.x = x;
    right = x + lastGap.width;
  }

  // The arc band, then the band under it.
  const arcTop = gridBottom + (orders.length > 0 ? ARC_PAD : 0);
  const arcBottom = arcTop + Math.max(0, tracks - 1) * TRACK_STEP + (tracks > 0 ? ARC_PAD : 0);
  const trackY = (track: number) => arcTop + track * TRACK_STEP;
  let bandY: number | null = null;
  let bottom = gridBottom;
  if (tracks > 0) {
    bottom = arcBottom;
  }
  if (band.length > 0) {
    bandY = bottom + (orders.length > 0 ? 6 : 0);
    let top = bandY + BAND_HEAD;
    for (const shelf of band) {
      const tallest = Math.max(...shelf.map((cell) => cell.height + (cell.loops ? LOOP_ROOM : 0)));
      for (const cell of shelf) {
        cell.y = top;
      }
      top += tallest + BAND_ROW_GAP;
    }
    bottom = top - BAND_ROW_GAP;
    const widths = Array.from({ length: bandColumns }, (_, column) => Math.max(0, ...inBand.filter((cell) => cell.bandColumn === column).map((cell) => cell.width)));
    let bandX = 0;
    widths.forEach((width, column) => {
      const gutter = gutters[column] ?? emptyChannel();
      gutter.width = column === 0 ? channelWidth(gutter, LANE_PAD) : channelWidth(gutter, BAND_GAP);
      gutter.x = bandX;
      bandX += gutter.width;
      for (const cell of inBand) {
        if (cell.bandColumn === column) {
          cell.x = bandX;
        }
      }
      bandX += width;
    });
    const lastGutter = gutters[bandColumns];
    if (lastGutter !== undefined) {
      lastGutter.width = channelWidth(lastGutter, LANE_PAD);
      lastGutter.x = bandX;
      right = Math.max(right, bandX + lastGutter.width);
    }
  }

  // Ports: a box's edges spread along its side in the order they leave, so none crosses at the box.
  const rightSides = new Sides();
  const leftSides = new Sides();
  for (const laid of edges) {
    if (laid.route === "forward") {
      const next = laid.chain[1];
      const before = laid.chain[laid.chain.length - 2];
      rightSides.add(laid.from, laid.id, next === undefined ? 0 : middleOf(next));
      leftSides.add(laid.to, laid.id, before === undefined ? 0 : middleOf(before));
    } else if (laid.route === "beside") {
      rightSides.add(laid.from, laid.id, laid.to.y);
      rightSides.add(laid.to, laid.id, laid.from.y);
    } else if (laid.direct) {
      rightSides.add(laid.from, laid.id, laid.to.y);
      leftSides.add(laid.to, laid.id, laid.from.y);
    } else if (laid.route === "arc" || laid.route === "band") {
      rightSides.add(laid.from, laid.outKey, BELOW_THE_ROWS - laid.track);
      leftSides.add(laid.to, laid.inKey, BELOW_THE_ROWS - laid.track);
    }
  }

  const shaped = edges.map((laid) => {
    if (laid.route === "self") {
      const loop = loopPath(laid.from);
      return { laid, shape: { box: laid.from }, label: { x: loop.middle.x, y: loop.middle.y + LOOP_LABEL_DROP } };
    }
    if (laid.route === "forward") {
      const runs = laid.chain.map((slot, index): Run => {
        if (index === 0) {
          const y = rightSides.y(laid.from, laid.id);
          return { from: laid.from.x + laid.from.width, to: laid.from.x + (columnWidthOf(orders, laid.from.column) ?? laid.from.width), y };
        }
        if (index === laid.chain.length - 1) {
          return { from: laid.to.x, to: laid.to.x, y: leftSides.y(laid.to, laid.id) };
        }
        const column = (laid.from.column ?? 0) + index;
        const left = columnLeftOf(orders, column);
        return { from: left, to: left + (columnWidthOf(orders, column) ?? 0), y: middleOf(slot) };
      });
      return { laid, shape: { runs }, label: curveMiddle(runs) };
    }
    if (laid.direct) {
      const y = rightSides.y(laid.from, laid.id);
      const runs: Run[] = [
        { from: laid.from.x + laid.from.width, to: laid.from.x + laid.from.width, y },
        { from: laid.to.x, to: laid.to.x, y: leftSides.y(laid.to, laid.id) },
      ];
      return { laid, shape: { runs }, label: curveMiddle(runs) };
    }
    if (laid.route === "beside") {
      const channelX = slotX(laid.outChannel, laid.id);
      const fromY = rightSides.y(laid.from, laid.id);
      const toY = rightSides.y(laid.to, laid.id);
      const points = [
        { x: laid.from.x + laid.from.width, y: fromY },
        { x: channelX, y: fromY },
        { x: channelX, y: toY },
        { x: laid.to.x + laid.to.width, y: toY },
      ];
      return { laid, shape: { points }, label: { x: channelX, y: (fromY + toY) / 2 } };
    }
    const outX = slotX(laid.outChannel, laid.outKey);
    const inX = slotX(laid.inChannel, laid.inKey);
    const fromY = rightSides.y(laid.from, laid.outKey);
    const toY = leftSides.y(laid.to, laid.inKey);
    const y = trackY(laid.track);
    const points = [
      { x: laid.from.x + laid.from.width, y: fromY },
      { x: outX, y: fromY },
      { x: outX, y },
      { x: inX, y },
      { x: inX, y: toY },
      { x: laid.to.x, y: toY },
    ];
    return { laid, shape: { points }, label: { x: (outX + inX) / 2, y } };
  });

  spreadLabels(shaped);

  return {
    cells,
    edges: shaped,
    columns: orders.map((slots) => slots.flatMap((slot) => (slot.cell === null ? [] : [slot.cell]))),
    band,
    bandY,
    width: Math.max(right, BOX_WIDTH + LANE_PAD * 2),
    height: bottom + LANE_PAD,
  };
}

/** How much room an edge's words take: a word wraps within the label's width. */
function labelSize(edge: FlowEdge): { width: number; height: number } {
  const lengths = edgeWords(edge).map((word) => Array.from(word).length * LABEL_CHARACTER);
  const lines = lengths.reduce((sum, length) => sum + Math.max(1, Math.ceil(length / LABEL_WIDTH)), 0);
  return { width: Math.min(LABEL_WIDTH, Math.max(0, ...lengths)) + 12, height: lines * LABEL_LINE + 4 };
}

/** Moves a label down off the ones before it: two edges between one pair of boxes say both. */
function spreadLabels(edges: readonly { laid: LaneEdge; label: Point }[]): void {
  const placed: Rect[] = [];
  for (const { laid, label } of edges) {
    const { width, height } = labelSize(laid.edge);
    if (height <= 4) {
      continue;
    }
    for (let hit = true; hit; ) {
      hit = false;
      for (const other of placed) {
        if (Math.abs(other.x - label.x) * 2 < other.width + width && Math.abs(other.y - label.y) * 2 < other.height + height) {
          label.y = other.y + (other.height + height) / 2 + 2;
          hit = true;
        }
      }
    }
    placed.push({ x: label.x, y: label.y, width, height });
  }
}

function columnWidthOf(orders: readonly Slot[][], column: number | null): number | undefined {
  if (column === null) {
    return undefined;
  }
  return Math.max(BOX_WIDTH, ...(orders[column] ?? []).map((slot) => slot.cell?.width ?? 0));
}

function columnLeftOf(orders: readonly Slot[][], column: number): number {
  const cell = (orders[column] ?? []).find((slot) => slot.cell !== null)?.cell;
  return cell?.x ?? 0;
}

/** Writes a lane and what it holds at absolute positions. */
/** The read's layout: the lane's boxes and edges at absolute positions, the lane at the origin. Pure and deterministic. */
export function layoutFlow(view: FlowView): FlowLayout {
  const { drawn, total } = drawnOf(view);
  const draft = draftLane(view, drawn);
  const layout: FlowLayout = {
    boxes: draft.cells.map((cell) => ({ id: cell.id, name: cell.name, node: cell.node, column: cell.column, row: cell.row, x: cell.x, y: cell.y, width: cell.width, height: cell.height })),
    lanes: [
      {
        x: 0,
        y: 0,
        width: draft.width,
        height: draft.height,
        columns: draft.columns.map((cells) => cells.map((cell) => cell.id)),
        band: draft.band.map((cells) => cells.map((cell) => cell.id)),
        bandY: draft.bandY,
      },
    ],
    edges: draft.edges.map(({ laid, shape, label }): LaidEdge => {
      if ("runs" in shape) {
        return { ...laidOf(laid, label), path: curvePath(shape.runs), points: curvePoints(shape.runs) };
      }
      if ("points" in shape) {
        return { ...laidOf(laid, label), path: roundedPath(shape.points), points: shape.points };
      }
      const loop = loopPath(shape.box);
      return { ...laidOf(laid, label), path: loop.path, points: loop.points };
    }),
    width: draft.width,
    height: draft.height,
    total,
    notDrawn: total - drawn.length,
  };
  return layout;
}

/** A laid edge's own fields, its path and points aside. */
function laidOf(laid: LaneEdge, label: Point): Omit<LaidEdge, "path" | "points"> {
  return { id: laid.id, edge: laid.edge, source: laid.from.id, target: laid.to.id, route: laid.route, label };
}

function nearestBy(ids: readonly string[], boxes: ReadonlyMap<string, FlowBox>, at: number, along: (box: FlowBox) => number): string | null {
  let best: string | null = null;
  let distance = Infinity;
  for (const id of ids) {
    const box = boxes.get(id);
    if (box !== undefined && Math.abs(along(box) - at) < distance) {
      best = id;
      distance = Math.abs(along(box) - at);
    }
  }
  return best;
}

/**
 * The box a key moves to from `from`, or null to stay. Up and Down keep to a column, then the band
 * under it; Left and Right go to the nearest box of the column beside, and along the band.
 */
export function boxAfterKey(layout: FlowLayout, from: string, key: string): string | null {
  const boxes = new Map(layout.boxes.map((box) => [box.id, box]));
  const box = boxes.get(from);
  const lane = layout.lanes[0];
  if (box === undefined || lane === undefined) {
    return null;
  }
  const midY = (other: FlowBox) => other.y + other.height / 2;
  const midX = (other: FlowBox) => other.x + other.width / 2;
  if (box.column !== null) {
    const column = lane.columns[box.column] ?? [];
    const row = column.indexOf(from);
    switch (key) {
      case "ArrowUp":
        return column[row - 1] ?? null;
      case "ArrowDown":
        return column[row + 1] ?? nearestBy(lane.band[0] ?? [], boxes, midX(box), midX);
      case "ArrowLeft":
        return nearestBy(lane.columns[box.column - 1] ?? [], boxes, midY(box), midY);
      case "ArrowRight":
        return nearestBy(lane.columns[box.column + 1] ?? [], boxes, midY(box), midY);
      default:
        return null;
    }
  }
  const shelf = lane.band[box.row] ?? [];
  const at = shelf.indexOf(from);
  switch (key) {
    case "ArrowLeft":
      return shelf[at - 1] ?? null;
    case "ArrowRight":
      return shelf[at + 1] ?? null;
    case "ArrowUp":
      return nearestBy(lane.band[box.row - 1] ?? lane.columns.flatMap((ids) => ids.slice(-1)), boxes, midX(box), midX);
    case "ArrowDown":
      return nearestBy(lane.band[box.row + 1] ?? [], boxes, midX(box), midX);
    default:
      return null;
  }
}
