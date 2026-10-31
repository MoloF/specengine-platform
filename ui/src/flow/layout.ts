import type { FlowEdge, FlowNode, FlowView } from "../api/types";
import { BOX_HEIGHT, BOX_WIDTH, CANVAS_LIMIT, compareText, nodeName } from "../graph/layout";
import { curveMiddle, curvePath, curvePoints, loopPath, LOOP_DEPTH, roundedPath, type Point, type Rect, type Run } from "./geometry";
import { edgeWords } from "./labels";

// The Flow's layout (docs/features/flow-canvas.md "Layout"), hand-written and pure: the reads in
// hand in, boxes, lanes and edges at absolute positions out. A lane per read, nested: a box's column
// is its `distance` in the read drawing it, rows come from four barycentre sweeps, and an open group
// is its lane under its head. Columns decide every edge's shape, never a type or a `kind`, so a loop
// nobody declared shows as well as one declared. Nothing here follows, lifts or counts a link: each
// edge drawn is its read's, unchanged. The input is sorted first, so the same reads in any order
// give the same picture.

export { BOX_HEIGHT, BOX_WIDTH, CANVAS_LIMIT };

/** An open group's head, over its lane. */
export const HEAD_HEIGHT = 44;
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

/** An open group is at least this wide, so its head can say what it is. */
const FRAME_MIN_WIDTH = 380;

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

export interface FlowInput {
  top: FlowView;
  /** The groups' reads in hand, by name. */
  groups: ReadonlyMap<string, FlowView>;
  /** The groups asked open. */
  open: ReadonlySet<string>;
}

/** A read as the screen shows it: Top, or an open group's. */
export interface DrawnRead {
  group: string | null;
  /** The read whose lane holds the group's head; null for Top and for a group of Top. */
  parent: string | null;
  nesting: number;
  view: FlowView;
  /** Its boxes as served; a group's own read holds the group, here left out. */
  boxes: FlowNode[];
}

/** A plain box, a closed group, or an open group: its head and frame. */
export type BoxForm = "step" | "closed" | "open";

export interface FlowBox extends Rect {
  /** `n:` and the name, the same box in every layout; a second holder of the name adds its path and line. */
  id: string;
  name: string;
  node: FlowNode;
  form: BoxForm;
  /** The group whose lane draws it; null at Top. */
  group: string | null;
  nesting: number;
  /** Its column in the lane; null in the band under it. */
  column: number | null;
  row: number;
  /** Asked open, read, and past the cap: it stays closed. */
  noRoom: boolean;
}

export interface FlowLane extends Rect {
  group: string | null;
  nesting: number;
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
  group: string | null;
  nesting: number;
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
  /** Boxes the reads in hand hold, drawn or not. */
  total: number;
  notDrawn: number;
}

/** A box's canvas id, a group's head's among them; a second holder of one ID adds its place. */
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

/** Whether the box can open: `holds > 0` is a closed group. */
export function isGroup(node: FlowNode): boolean {
  return node.holds > 0;
}

/**
 * The reads the screen shows: Top, then each open group with its read in hand, by (nesting, name),
 * reachable from Top through open groups. A read with a `reason` shows nothing: its group stays closed.
 */
export function readsOf(input: FlowInput): DrawnRead[] {
  const reads: DrawnRead[] = [{ group: null, parent: null, nesting: 0, view: input.top, boxes: [...input.top.nodes].sort(byServed) }];
  const seen = new Set<string>();
  const asked = (boxes: readonly FlowNode[], parent: string | null) => boxes.filter((node) => isGroup(node) && input.open.has(nodeName(node))).map((node) => ({ name: nodeName(node), parent }));
  let level = asked(reads[0]?.boxes ?? [], null);
  for (let nesting = 1; level.length > 0; nesting += 1) {
    const next: { name: string; parent: string | null }[] = [];
    for (const { name, parent } of [...level].sort((a, b) => compareText(a.name, b.name))) {
      const view = input.groups.get(name);
      if (seen.has(name) || view === undefined || view.reason !== null) {
        continue;
      }
      seen.add(name);
      const boxes = view.nodes.filter((node) => nodeName(node) !== name).sort(byServed);
      if (boxes.length > 0) {
        reads.push({ group: name, parent, nesting, view, boxes });
        next.push(...asked(boxes, name));
      }
    }
    level = next;
  }
  return reads;
}

/** Which boxes of which read the cap leaves on the canvas. */
interface Plan {
  reads: Map<string | null, DrawnRead>;
  drawn: Map<string | null, FlowNode[]>;
  noRoom: Set<string>;
  total: number;
  placed: number;
  /** The names a cell took while the lanes are drafted: a link lands on a name's first holder. */
  held: Set<string>;
}

/**
 * The cap: from Top as served, then open groups by (nesting, name), `CANVAS_LIMIT` boxes in all. A
 * group's boxes past it stay undrawn, with their edges; a group with no room left stays closed.
 */
function planOf(input: FlowInput): Plan {
  const reads = readsOf(input);
  const plan: Plan = {
    reads: new Map(reads.map((read) => [read.group, read])),
    drawn: new Map(),
    noRoom: new Set(),
    total: 0,
    placed: 0,
    held: new Set(),
  };
  const placed = new Set<string>();
  for (const read of reads) {
    plan.total += read.boxes.length;
    if (read.group !== null && !plan.drawn.get(read.parent)?.some((node) => nodeName(node) === read.group)) {
      continue;
    }
    const room = CANVAS_LIMIT - placed.size;
    if (room <= 0) {
      if (read.group !== null) {
        plan.noRoom.add(read.group);
      }
      continue;
    }
    const taken = read.boxes.filter((node) => !placed.has(placeOf(node))).slice(0, room);
    for (const node of taken) {
      placed.add(placeOf(node));
    }
    plan.drawn.set(read.group, taken);
  }
  plan.placed = placed.size;
  return plan;
}

/** A box in its lane while the lane is laid out, positions relative to the lane. */
interface Cell {
  id: string;
  name: string;
  node: FlowNode;
  inner: Draft | null;
  noRoom: boolean;
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
  group: string | null;
  nesting: number;
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

function edgeKey(group: string | null, edge: FlowEdge): string {
  return `e:${group ?? ""}:${edge.from}>${edge.to}:${edge.type}:${edge.kind ?? ""}`;
}

function linked(a: Slot, b: Slot): void {
  a.after.push(b);
  b.before.push(a);
}

/** A lane's boxes, opened ones holding their own lanes; only the cap's boxes. */
function cellsOf(group: string | null, nesting: number, plan: Plan): Cell[] {
  return (plan.drawn.get(group) ?? []).map((node): Cell => {
    const name = nodeName(node);
    const first = !plan.held.has(name);
    plan.held.add(name);
    const inner = first && plan.drawn.has(name) && isGroup(node) ? draftLane(name, nesting + 1, plan) : null;
    return {
      id: first ? boxId(name) : `${boxId(name)}@${placeOf(node)}`,
      name,
      node,
      inner,
      noRoom: plan.noRoom.has(name),
      width: inner === null ? BOX_WIDTH : Math.max(FRAME_MIN_WIDTH, inner.width),
      height: inner === null ? BOX_HEIGHT : HEAD_HEIGHT + inner.height,
      anchor: (inner === null ? BOX_HEIGHT : HEAD_HEIGHT) / 2,
      loops: false,
      column: null,
      row: 0,
      x: 0,
      y: 0,
      bandColumn: 0,
    };
  });
}

/** Lays out one read's lane, relative to its top left. */
function draftLane(group: string | null, nesting: number, plan: Plan): Draft {
  const cells = cellsOf(group, nesting, plan);
  // A link lands on a name's first holder.
  const byName = new Map([...cells].reverse().map((cell) => [cell.name, cell]));
  const layers = [...new Set(cells.flatMap((cell) => (cell.node.distance === null ? [] : [cell.node.distance])))].sort((a, b) => a - b);
  const columnOf = new Map(layers.map((layer, column) => [layer, column]));
  for (const cell of cells) {
    cell.column = cell.node.distance === null ? null : (columnOf.get(cell.node.distance) ?? null);
  }

  const read = plan.reads.get(group);
  const edges: LaneEdge[] = [...(read?.view.edges ?? [])].sort(byEdge).flatMap((edge) => {
    const from = byName.get(edge.from);
    const to = byName.get(edge.to);
    if (from === undefined || to === undefined) {
      return [];
    }
    const id = edgeKey(group, edge);
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
    group,
    nesting,
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

function moved(point: Point, by: Point): Point {
  return { x: point.x + by.x, y: point.y + by.y };
}

/** Writes a lane and what it holds at absolute positions. */
function placeLane(draft: Draft, origin: Point, into: FlowLayout): void {
  into.lanes.push({
    group: draft.group,
    nesting: draft.nesting,
    x: origin.x,
    y: origin.y,
    width: draft.width,
    height: draft.height,
    columns: draft.columns.map((cells) => cells.map((cell) => cell.id)),
    band: draft.band.map((cells) => cells.map((cell) => cell.id)),
    bandY: draft.bandY === null ? null : origin.y + draft.bandY,
  });
  for (const cell of draft.cells) {
    const x = origin.x + cell.x;
    const y = origin.y + cell.y;
    into.boxes.push({
      id: cell.id,
      name: cell.name,
      node: cell.node,
      form: cell.inner !== null ? "open" : isGroup(cell.node) ? "closed" : "step",
      group: draft.group,
      nesting: draft.nesting,
      column: cell.column,
      row: cell.row,
      noRoom: cell.noRoom,
      x,
      y,
      width: cell.width,
      height: cell.height,
    });
    if (cell.inner !== null) {
      placeLane(cell.inner, { x, y: y + HEAD_HEIGHT }, into);
    }
  }
  for (const { laid, shape, label } of draft.edges) {
    let path: string;
    let points: Point[];
    if ("runs" in shape) {
      const runs = shape.runs.map((run) => ({ from: run.from + origin.x, to: run.to + origin.x, y: run.y + origin.y }));
      path = curvePath(runs);
      points = curvePoints(runs);
    } else if ("points" in shape) {
      points = shape.points.map((point) => moved(point, origin));
      path = roundedPath(points);
    } else {
      const loop = loopPath({ x: shape.box.x + origin.x, y: shape.box.y + origin.y, width: shape.box.width, height: shape.box.height });
      path = loop.path;
      points = loop.points;
    }
    into.edges.push({
      id: laid.id,
      edge: laid.edge,
      group: draft.group,
      nesting: draft.nesting,
      source: laid.from.id,
      target: laid.to.id,
      route: laid.route,
      points,
      path,
      label: moved(label, origin),
    });
  }
}

/** Lays out the reads in hand. Pure and deterministic. */
export function layoutFlow(input: FlowInput): FlowLayout {
  const plan = planOf(input);
  const draft = draftLane(null, 0, plan);
  const layout: FlowLayout = {
    boxes: [],
    lanes: [],
    edges: [],
    width: draft.width,
    height: draft.height,
    total: plan.total,
    notDrawn: plan.total - plan.placed,
  };
  placeLane(draft, { x: 0, y: 0 }, layout);
  return layout;
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

/** The first box of a lane: its first column's top, else its band's first. */
function firstOf(lane: FlowLane): string | null {
  return lane.columns[0]?.[0] ?? lane.band[0]?.[0] ?? null;
}

/**
 * What lies right of a lane: the box beside its frame, climbing out of as many frames as end there.
 * Without it a box right of an open group could not be reached: Right on the head goes into the lane.
 */
function pastTheFrame(layout: FlowLayout, boxes: ReadonlyMap<string, FlowBox>, lane: FlowLane): string | null {
  const midY = (other: FlowBox) => other.y + Math.min(other.height, BOX_HEIGHT) / 2;
  for (let group = lane.group; group !== null; ) {
    const head = boxes.get(boxId(group));
    const outer = layout.lanes.find((candidate) => candidate.group === head?.group);
    if (head === undefined || outer === undefined) {
      return null;
    }
    const shelf = outer.band[head.row] ?? [];
    const next = head.column === null ? shelf[shelf.indexOf(head.id) + 1] : nearestBy(outer.columns[head.column + 1] ?? [], boxes, midY(head), midY);
    if (next !== undefined && next !== null) {
      return next;
    }
    group = outer.group;
  }
  return null;
}

/**
 * The box a key moves to from `from`, or null to stay. Up and Down keep to a column, then the band
 * under it; Left and Right go across: Left from a first column to the frame's head, Right on an open
 * head into its lane, Right from a lane's last column to what lies past its frame.
 */
export function boxAfterKey(layout: FlowLayout, from: string, key: string): string | null {
  const boxes = new Map(layout.boxes.map((box) => [box.id, box]));
  const box = boxes.get(from);
  const lane = layout.lanes.find((candidate) => candidate.group === box?.group);
  if (box === undefined || lane === undefined) {
    return null;
  }
  const midY = (other: FlowBox) => other.y + Math.min(other.height, BOX_HEIGHT) / 2;
  const midX = (other: FlowBox) => other.x + Math.min(other.width, BOX_WIDTH) / 2;
  const head = lane.group === null ? null : boxId(lane.group);
  if (key === "ArrowRight" && box.form === "open") {
    const inner = layout.lanes.find((candidate) => candidate.group === box.name);
    return inner === undefined ? null : firstOf(inner);
  }
  if (box.column !== null) {
    const column = lane.columns[box.column] ?? [];
    const row = column.indexOf(from);
    switch (key) {
      case "ArrowUp":
        return column[row - 1] ?? null;
      case "ArrowDown":
        return column[row + 1] ?? nearestBy(lane.band[0] ?? [], boxes, midX(box), midX);
      case "ArrowLeft":
        return box.column === 0 ? head : nearestBy(lane.columns[box.column - 1] ?? [], boxes, midY(box), midY);
      case "ArrowRight":
        return nearestBy(lane.columns[box.column + 1] ?? [], boxes, midY(box), midY) ?? pastTheFrame(layout, boxes, lane);
      default:
        return null;
    }
  }
  const shelf = lane.band[box.row] ?? [];
  const at = shelf.indexOf(from);
  switch (key) {
    case "ArrowLeft":
      return shelf[at - 1] ?? head;
    case "ArrowRight":
      return shelf[at + 1] ?? pastTheFrame(layout, boxes, lane);
    case "ArrowUp":
      return nearestBy(lane.band[box.row - 1] ?? lane.columns.flatMap((ids) => ids.slice(-1)), boxes, midX(box), midX);
    case "ArrowDown":
      return nearestBy(lane.band[box.row + 1] ?? [], boxes, midX(box), midX);
    default:
      return null;
  }
}

/** The nearest box still drawn that holds `id`, walking up the layout it was drawn in. */
export function enclosingHead(before: FlowLayout, after: FlowLayout, id: string): string | null {
  const boxes = new Map(before.boxes.map((box) => [box.id, box]));
  const drawn = new Set(after.boxes.map((box) => box.id));
  let group = boxes.get(id)?.group ?? null;
  while (group !== null) {
    const head = boxId(group);
    if (drawn.has(head)) {
      return head;
    }
    group = boxes.get(head)?.group ?? null;
  }
  return null;
}
