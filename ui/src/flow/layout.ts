import type { FlowEdge, FlowNode, FlowView } from "../api/types";
import { CANVAS_LIMIT, compareText, nodeName } from "../graph/layout";
import { BOX_HEIGHT, BOX_WIDTH, curveMiddle, curvePath, curvePoints, loopPath, LOOP_DEPTH, roundedPath, type Point, type Rect, type Run } from "./geometry";
import { edgeWords } from "./labels";

// The Flow's layout (`docs/canon/spec-flow-layout.md` "Layout"; docs/features/handoff-progress.md
// "Acceptance criteria", F-1 to F-3), hand-written and pure: one read in (a group is entered, never
// opened in place), boxes, its lane, its stage dividers and edges at absolute positions out. The walk
// reads left to right: a box's column is its `distance` in the read, its place in the column comes
// from four barycentre sweeps; stage dividers stand left of the columns where stages begin; an edge
// against the walk runs on its own track below the walk, and the boxes off the walk wrap in a band
// below the tracks. Columns decide every edge's shape, never a type or a `kind`, so a loop nobody
// declared shows as well as one declared. Nothing here follows, lifts or counts a link: each edge
// drawn is the read's, unchanged. The input is sorted first, so the same read in any order gives the
// same picture.

export { BOX_HEIGHT, BOX_WIDTH, CANVAS_LIMIT };

/**
 * The band between two columns at least: an edge label of two lines at most (`LABEL_WIDTH`) and its
 * insets, so a column's pitch is 256 px, 235.5 px at the fit floor
 * (docs/features/room-and-scroll.md "Acceptance criteria", RS-1, RS-2).
 */
export const COLUMN_GAP = 96;
/** Between two boxes of a column. */
export const BOX_GAP = 48;
/** Inside a lane, each side. */
export const LANE_PAD = 16;
/** From one arc's track to the next, and one trunk's. */
export const TRACK_STEP = 12;
/** From one stretch to the next in a gap. */
export const SLOT_STEP = 8;
/** From one port to the next on a box's side. */
export const PORT_STEP = 8;
/** A stage divider's words strip above the walk: two lines of the small size and their clearance. */
export const DIVIDER_HEIGHT = 40;
/** The band caption's least width: a picture with a band is at least this wide past the walk's left, so the caption stays inside it. */
export const BAND_CAPTION_WIDTH = 320;

/** What a long forward edge reserves in a column it passes, and its distance to what is beside it. */
const PASSAGE_HEIGHT = 2;
const PASSAGE_GAP = 12;
/** Kept free under a box with an edge to itself: the loop and its words under it. */
const LOOP_LABEL_DROP = 16;
const LOOP_ROOM = LOOP_DEPTH + LOOP_LABEL_DROP + 20;
/** From the walk's bottom to its first track; from a gap's side to its first stretch. */
const ARC_PAD = 24;
const GAP_INSET = 12;
/** A divider's rule stands this far left of its column's boxes. */
const DIVIDER_INSET = 6;
/** The band's caption line, the space between its boxes and its shelves, its columns at least. */
const BAND_HEAD = 40;
const BAND_GAP = 20;
const BAND_SHELF_GAP = 24;
const BAND_MIN_COLUMNS = 4;
/** Passes that align each column with its neighbours once the order is set. */
const PLACE_PASSES = 8;
/**
 * An edge label's width at most, the column gap less 4 px each side (`--flow-edge-label-width` in
 * tokens.css), about 12 characters a line; its lines at most, its line height, and a character's mean
 * width, for telling two apart.
 */
export const LABEL_WIDTH = 88;
const LABEL_LINES = 2;
const LABEL_LINE = 16;
const LABEL_CHARACTER = 6;
/** Below every port a forward edge takes on a side: an arc or a band edge leaves and enters there. */
const BELOW_THE_WALK = 1_000_000;

export interface FlowBox extends Rect {
  /** `n:` and the name, the same box in every layout; a second holder of the name adds its path and line. */
  id: string;
  name: string;
  node: FlowNode;
  /** Its column on the walk; null in the band. */
  level: number | null;
  /** Its place in its column, top to bottom, or in its band shelf, left to right. */
  order: number;
  /** Its band shelf; 0 on the walk. */
  shelf: number;
}

/** A stage divider: a vertical rule left of the column where its stages begin, words raw at its top. */
export interface FlowDivider {
  id: string;
  level: number;
  /** The stages that begin in this column, in column order. */
  stages: string[];
  /** The rule: left of its column's boxes, right of the previous column's. */
  x: number;
  /** The top of its words' strip, above every walk box. */
  y: number;
  /** From the rule to its column's right edge: the room its words wrap in. */
  width: number;
  /** From the top of its words' strip to below the lowest arc track. */
  height: number;
}

export interface FlowLane extends Rect {
  /** Each walk column's box ids, top to bottom. */
  columns: string[][];
  /** The band's shelves of box ids, left to right. */
  band: string[][];
  /** Where the band's caption sits; null with no band. */
  bandY: number | null;
  dividers: FlowDivider[];
}

/** Forward along the walk, against it, within a column, to itself, or to or from the band. */
export type EdgeRoute = "forward" | "arc" | "beside" | "self" | "band";

export interface LaidEdge {
  id: string;
  /** The read's edge, unchanged. */
  edge: FlowEdge;
  source: string;
  target: string;
  route: EdgeRoute;
  /** Either end off the walk: drawn only while an end, or the edge, is selected. */
  offWalk: boolean;
  /** The corners of an arc, the ends of a curve's runs. */
  points: Point[];
  path: string;
  label: Point;
}

export interface FlowLayout {
  /** Drawn boxes. */
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

/** A box while the lane is laid out. */
interface Cell {
  id: string;
  name: string;
  node: FlowNode;
  width: number;
  height: number;
  loops: boolean;
  level: number | null;
  order: number;
  shelf: number;
  /** Its band column, for a box in the band. */
  column: number;
  x: number;
  y: number;
}

/** A place in a column: a box, or the passage of a long forward edge. */
interface Slot {
  cell: Cell | null;
  /** Its height in the column: a box's own and its loop's room, or a passage's. */
  height: number;
  rank: number;
  order: number;
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
  /** Its track below the walk (an arc), or its trunk over the band (a band edge with a walk end); -1 for none. */
  track: number;
  /** Two boxes of a column or a shelf next to each other: a line from side to side. */
  direct: boolean;
  /** What its band end shares with edges of the same type and kind at that box: trunk, gutter, shelf stretch, port. */
  bandKey: string;
}

/** A barycentre as a fraction, compared without rounding. */
interface Key {
  sum: number;
  count: number;
}

function compareKeys(a: Key, b: Key): number {
  return a.sum * b.count - b.sum * a.count;
}

/** Reorders a column by the mean place of each slot's neighbours; a slot with none keeps its place. */
function sweepColumn(slots: readonly Slot[], near: (slot: Slot) => readonly Slot[]): Slot[] {
  const keys = new Map<Slot, Key>();
  for (const slot of slots) {
    const others = near(slot);
    keys.set(slot, others.length === 0 ? { sum: slot.order, count: 1 } : { sum: others.reduce((sum, other) => sum + other.order, 0), count: others.length });
  }
  const sorted = [...slots].sort((a, b) => compareKeys(keys.get(a) ?? { sum: 0, count: 1 }, keys.get(b) ?? { sum: 0, count: 1 }) || a.rank - b.rank);
  sorted.forEach((slot, order) => {
    slot.order = order;
  });
  return sorted;
}

/**
 * Tops as near as can be to `wanted`, the order kept, `gaps[i]` between items i and i + 1:
 * subtracting each item's least offset turns the spacing into "never decreasing", which pooling
 * adjacent violators solves exactly.
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

/** A slot's middle: a box's own, its loop's room aside. */
function middleOf(slot: Slot): number {
  return slot.y + (slot.cell === null ? slot.height : slot.cell.height) / 2;
}

/** Aligns one column with its neighbours on `near`; a slot with none stays where it is. */
function placeColumn(slots: readonly Slot[], near: (slot: Slot) => readonly Slot[]): void {
  const wanted = slots.map((slot) => {
    const others = near(slot);
    const own = slot.cell === null ? slot.height : slot.cell.height;
    return others.length === 0 ? slot.y : others.reduce((sum, other) => sum + middleOf(other), 0) / others.length - own / 2;
  });
  const gaps = slots.map((slot, index) => {
    const next = slots[index + 1];
    return slot.cell === null || next?.cell === null ? PASSAGE_GAP : BOX_GAP;
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

function linked(a: Slot, b: Slot): void {
  a.after.push(b);
  b.before.push(a);
}

function slotOf(cell: Cell, rank: number): Slot {
  return { cell, height: cell.height + (cell.loops ? LOOP_ROOM : 0), rank, order: rank, y: 0, before: [], after: [] };
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
      loops: false,
      level: null,
      order: 0,
      shelf: 0,
      column: 0,
      x: 0,
      y: 0,
    };
  });
}

function routeOf(from: Cell, to: Cell): EdgeRoute {
  if (from === to) {
    return "self";
  }
  if (from.level === null || to.level === null) {
    return "band";
  }
  if (to.level < from.level) {
    return "arc";
  }
  return to.level === from.level ? "beside" : "forward";
}

function edgeKey(edge: FlowEdge): string {
  return `e:${edge.from}>${edge.to}:${edge.type}:${edge.kind ?? ""}`;
}

/** Adds to a cell's list, the list made once: no copy per entry. */
function addTo<T>(map: Map<Cell, T[]>, cell: Cell, value: T): void {
  const held = map.get(cell);
  if (held === undefined) {
    map.set(cell, [value]);
  } else {
    held.push(value);
  }
}

/**
 * The band's order: under the walk boxes each links to, those linked to none last; a box followed at
 * once by what it leads to, so a chain off the walk reads as one, left to right.
 */
function bandOrderOf(cells: readonly Cell[], edges: readonly LaneEdge[]): Cell[] {
  const places = new Map<Cell, number[]>();
  const leads = new Map<Cell, Cell[]>();
  const led = new Set<Cell>();
  for (const { from, to } of edges) {
    if (from.level === null && to.level !== null) {
      addTo(places, from, to.x);
    } else if (to.level === null && from.level !== null) {
      addTo(places, to, from.x);
    } else if (from.level === null && to.level === null && from !== to) {
      addTo(leads, from, to);
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

/** The vertical stretches of one gap between columns: out of the column on its left, from its left side; into the column on its right, from its right side. */
class Channel {
  private readonly outs = new Map<string, number>();
  private readonly ins = new Map<string, number>();
  left = 0;
  right = 0;

  out(key: string): void {
    if (!this.outs.has(key)) {
      this.outs.set(key, this.outs.size);
    }
  }

  into(key: string): void {
    if (!this.ins.has(key)) {
      this.ins.set(key, this.ins.size);
    }
  }

  outX(key: string): number {
    return this.left + GAP_INSET + (this.outs.get(key) ?? 0) * SLOT_STEP;
  }

  inX(key: string): number {
    return this.right - GAP_INSET - (this.ins.get(key) ?? 0) * SLOT_STEP;
  }

  get outCount(): number {
    return this.outs.size;
  }

  get inCount(): number {
    return this.ins.size;
  }
}

/** The horizontal stretches of one band shelf's gap, each its own height, in the order asked. */
class ShelfGap {
  private readonly slots = new Map<string, number>();
  top = 0;

  ask(key: string): void {
    if (!this.slots.has(key)) {
      this.slots.set(key, this.slots.size);
    }
  }

  /** A stretch's height: from the gap's top, one step each. */
  y(key: string): number {
    return this.top + GAP_INSET + (this.slots.get(key) ?? 0) * SLOT_STEP;
  }

  /** The height its stretches take. */
  get room(): number {
    return this.slots.size === 0 ? 0 : GAP_INSET + this.slots.size * SLOT_STEP;
  }
}

/** One side of each box: who ends there, in what order, at what offset from the side's middle. */
class Sides {
  private readonly uses = new Map<Cell, Map<string, number>>();

  add(cell: Cell, key: string, order: number): void {
    const side = this.uses.get(cell) ?? new Map<string, number>();
    if (!side.has(key)) {
      side.set(key, order);
    }
    this.uses.set(cell, side);
  }

  /** The port's offset from the side's middle, along a side `length` long. */
  offset(cell: Cell, key: string, length: number): number {
    const side = this.uses.get(cell);
    if (side === undefined || side.size < 2) {
      return 0;
    }
    const ordered = [...side.entries()].sort(([ka, a], [kb, b]) => a - b || compareText(ka, kb)).map(([name]) => name);
    const room = (length - 16) / (ordered.length - 1);
    const step = Math.min(PORT_STEP, room);
    return (ordered.indexOf(key) - (ordered.length - 1) / 2) * step;
  }
}

/** The stage dividers of the walk's columns: a stage's divider stands left of the first column holding a box of it; fewer than two stages, none. */
function dividersOf(columns: readonly Cell[][]): { level: number; stages: string[] }[] {
  const begun = new Map<string, number>();
  columns.forEach((column, level) => {
    for (const cell of column) {
      const stage = cell.node.stage;
      if (stage !== null && !begun.has(stage)) {
        begun.set(stage, level);
      }
    }
  });
  if (begun.size < 2) {
    return [];
  }
  const byLevel = new Map<number, string[]>();
  for (const [stage, level] of begun) {
    const stages = byLevel.get(level);
    if (stages === undefined) {
      byLevel.set(level, [stage]);
    } else {
      stages.push(stage);
    }
  }
  return [...byLevel.entries()].sort(([a], [b]) => a - b).map(([level, stages]) => ({ level, stages }));
}

/** The read's layout: the lane's boxes and edges at absolute positions, the lane at the origin. Pure and deterministic. */
export function layoutFlow(view: FlowView): FlowLayout {
  const { drawn, total } = drawnOf(view);
  const cells = cellsOf(drawn);
  // A link lands on a name's first holder.
  const byName = new Map([...cells].reverse().map((cell) => [cell.name, cell]));
  const layers = [...new Set(cells.flatMap((cell) => (cell.node.distance === null ? [] : [cell.node.distance])))].sort((a, b) => a - b);
  const levelOf = new Map(layers.map((layer, level) => [layer, level]));
  for (const cell of cells) {
    cell.level = cell.node.distance === null ? null : (levelOf.get(cell.node.distance) ?? null);
  }

  const edges: LaneEdge[] = [...view.edges].sort(byEdge).flatMap((edge) => {
    const from = byName.get(edge.from);
    const to = byName.get(edge.to);
    if (from === undefined || to === undefined) {
      return [];
    }
    return [{ edge, id: edgeKey(edge), from, to, route: routeOf(from, to), chain: [], track: -1, direct: false, bandKey: "" }];
  });
  for (const laid of edges) {
    if (laid.route === "self") {
      laid.from.loops = true;
    }
  }

  // Columns, a passage for each column a forward edge skips; four sweeps.
  const columns: Slot[][] = layers.map(() => []);
  const slotByCell = new Map<Cell, Slot>();
  let rank = 0;
  for (const cell of cells) {
    if (cell.level !== null) {
      const slot = slotOf(cell, rank);
      rank += 1;
      slotByCell.set(cell, slot);
      columns[cell.level]?.push(slot);
    }
  }
  for (const laid of edges) {
    const source = slotByCell.get(laid.from);
    const target = slotByCell.get(laid.to);
    if (laid.route !== "forward" || source === undefined || target === undefined || laid.from.level === null || laid.to.level === null) {
      continue;
    }
    const chain = [source];
    for (let level = laid.from.level + 1; level < laid.to.level; level += 1) {
      const passage: Slot = { cell: null, height: PASSAGE_HEIGHT, rank, order: rank, y: 0, before: [], after: [] };
      rank += 1;
      columns[level]?.push(passage);
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
    slots.forEach((slot, order) => {
      slot.order = order;
    });
  }
  const orders = columns.map((slots) => slots);
  const sweep = (forth: boolean) => {
    const sequence = orders.map((_, index) => (forth ? index : orders.length - 1 - index)).slice(1);
    for (const at of sequence) {
      orders[at] = sweepColumn(orders[at] ?? [], (slot) => (forth ? slot.before : slot.after));
    }
  };
  sweep(true);
  sweep(false);
  sweep(true);
  sweep(false);

  // Each column stacked top to bottom, then pulled towards its neighbours: a join sits between its branches.
  for (const slots of orders) {
    let top = 0;
    slots.forEach((slot, index) => {
      slot.y = top;
      const next = slots[index + 1];
      top += slot.height + (slot.cell === null || next?.cell === null ? PASSAGE_GAP : BOX_GAP);
    });
  }
  for (let pass = 0; pass < PLACE_PASSES; pass += 1) {
    const forth = pass % 2 === 0;
    const sequence = orders.map((_, index) => (forth ? index : orders.length - 1 - index));
    for (const at of sequence) {
      placeColumn(orders[at] ?? [], (slot) => (forth ? slot.before : slot.after));
    }
  }
  const walkColumns = orders.map((slots) => slots.flatMap((slot) => (slot.cell === null ? [] : [slot.cell])));
  walkColumns.forEach((column, level) => {
    column.forEach((cell, order) => {
      cell.level = level;
      cell.order = order;
    });
  });

  // The words' strip above the walk, then the walk under it.
  const dividerColumns = dividersOf(walkColumns);
  const walkTop = LANE_PAD + (dividerColumns.length > 0 ? DIVIDER_HEIGHT : 0);
  const highest = Math.min(0, ...orders.flatMap((slots) => slots.map((slot) => slot.y)));
  let walkBottom = walkTop;
  for (const slots of orders) {
    for (const slot of slots) {
      slot.y = Math.round(slot.y - highest) + walkTop;
      walkBottom = Math.max(walkBottom, slot.y + slot.height);
      if (slot.cell !== null) {
        slot.cell.y = slot.y;
      }
    }
  }
  /** The lowest box of each column: an arc leaves or enters it by its bottom. */
  const lowest = new Set(walkColumns.flatMap((column) => column.slice(-1)));

  // Arcs below the walk: the shortest nearest it, so longer ones wrap them.
  const span = (laid: LaneEdge) => Math.abs((laid.from.level ?? 0) - (laid.to.level ?? 0));
  const arcs = edges.filter((laid) => laid.route === "arc").sort((a, b) => span(a) - span(b) || compareText(a.id, b.id));
  arcs.forEach((laid, track) => {
    laid.track = track;
  });

  // Next to each other in a column: a line from one box to the next.
  for (const laid of edges) {
    if (laid.route === "beside" && Math.abs(laid.from.order - laid.to.order) === 1) {
      laid.direct = true;
    }
  }

  // The gaps' stretches: gap g stands left of column g, the last right of the last column.
  const channels = Array.from({ length: layers.length + 1 }, () => new Channel());
  const rightOf = (cell: Cell) => channels[(cell.level ?? 0) + 1] ?? new Channel();
  const leftOf = (cell: Cell) => channels[cell.level ?? 0] ?? new Channel();
  for (const laid of edges) {
    const { from, to } = laid;
    if (laid.route === "beside" && !laid.direct) {
      rightOf(from).out(laid.id);
    } else if (laid.route === "arc") {
      if (!lowest.has(from)) {
        rightOf(from).out(`${laid.id}:out`);
      }
      if (!lowest.has(to)) {
        leftOf(to).into(`${laid.id}:in`);
      }
    } else if (laid.route === "band") {
      if (from.level !== null) {
        rightOf(from).out(`${laid.id}:out`);
      }
      if (to.level !== null) {
        leftOf(to).into(`${laid.id}:in`);
      }
    }
  }

  // Columns left to right, each gap as wide as its stretches need.
  const columnX: number[] = [];
  let x = 0;
  channels.forEach((channel, at) => {
    const stretches = channel.outCount + channel.inCount;
    const room = stretches === 0 ? 0 : 2 * GAP_INSET + (stretches - 1) * SLOT_STEP;
    const width = at === 0 ? LANE_PAD + (channel.inCount === 0 ? 0 : GAP_INSET + channel.inCount * SLOT_STEP) : at === layers.length ? (channel.outCount === 0 ? 0 : GAP_INSET + channel.outCount * SLOT_STEP) + LANE_PAD : Math.max(COLUMN_GAP, room);
    channel.left = x;
    channel.right = x + width;
    x += width;
    if (at < layers.length) {
      columnX.push(x);
      for (const cell of walkColumns[at] ?? []) {
        cell.x = x;
      }
      x += BOX_WIDTH;
    }
  });
  const walkLeft = columnX[0] ?? LANE_PAD;
  const walkRight = layers.length === 0 ? walkLeft : (columnX[layers.length - 1] ?? 0) + BOX_WIDTH;

  // The band: its boxes under the walk boxes each links to, wrapped at the walk's width or four boxes.
  const inBand = cells.filter((cell) => cell.level === null);
  const walkWidth = layers.length === 0 ? 0 : walkRight - walkLeft;
  const fitting = Math.floor((walkWidth + BAND_GAP) / (BOX_WIDTH + BAND_GAP));
  const bandColumns = Math.max(1, Math.min(inBand.length, Math.max(fitting, BAND_MIN_COLUMNS)));
  const band: Cell[][] = [];
  bandOrderOf(inBand, edges).forEach((cell, index) => {
    const shelf = Math.floor(index / bandColumns);
    cell.shelf = shelf;
    cell.order = index % bandColumns;
    cell.column = index % bandColumns;
    cell.x = walkLeft + cell.column * (BOX_WIDTH + BAND_GAP);
    const row = band[shelf] ?? [];
    row.push(cell);
    band[shelf] = row;
  });
  for (const laid of edges) {
    const { from, to } = laid;
    if (laid.route === "band" && from.level === null && to.level === null && from.shelf === to.shelf && Math.abs(from.order - to.order) === 1) {
      laid.direct = true;
    }
  }

  // Trunks over the band: an edge with a walk end crosses to its band end's gutter on one, shared by
  // the edges of one type and kind at that band box; each shelf's gap holds its stretches.
  const trunks = new Map<string, number>();
  const shelfGaps = Array.from({ length: band.length }, () => new ShelfGap());
  const shelfGap = (cell: Cell) => shelfGaps[cell.shelf] ?? new ShelfGap();
  for (const laid of edges) {
    if (laid.route !== "band" || laid.direct) {
      continue;
    }
    const { from, to } = laid;
    if (from.level === null && to.level === null) {
      shelfGap(from).ask(`${laid.id}:out`);
      shelfGap(to).ask(`${laid.id}:in`);
      continue;
    }
    const bandEnd = to.level === null ? to : from;
    laid.bandKey = `${bandEnd.id}:${laid.edge.type}:${laid.edge.kind ?? ""}`;
    if (!trunks.has(laid.bandKey)) {
      trunks.set(laid.bandKey, trunks.size);
    }
    laid.track = trunks.get(laid.bandKey) ?? 0;
    shelfGap(bandEnd).ask(laid.bandKey);
  }

  // Heights below the walk: its tracks, the trunks, then the band under its caption.
  const hasWalk = layers.length > 0;
  const trackTop = walkBottom + ARC_PAD;
  const trackY = (track: number) => trackTop + track * TRACK_STEP;
  const ruleBottom = arcs.length > 0 ? trackY(arcs.length - 1) + GAP_INSET : walkBottom + GAP_INSET;
  let y = hasWalk ? ruleBottom : LANE_PAD;
  const trunkTop = y;
  const trunkY = (track: number) => trunkTop + GAP_INSET + track * TRACK_STEP;
  if (trunks.size > 0) {
    y = trunkY(trunks.size - 1) + GAP_INSET;
  }
  let bandY: number | null = null;
  if (band.length > 0) {
    bandY = y + (hasWalk ? 8 : 0);
    y = bandY + BAND_HEAD;
    band.forEach((shelf, index) => {
      const gap = shelfGaps[index] ?? new ShelfGap();
      gap.top = y - (index === 0 ? 0 : BAND_SHELF_GAP);
      y = Math.max(y, gap.top + gap.room);
      const tallest = Math.max(...shelf.map((cell) => cell.height + (cell.loops ? LOOP_ROOM : 0)));
      for (const cell of shelf) {
        cell.y = y;
      }
      y += tallest + BAND_SHELF_GAP;
    });
    y -= BAND_SHELF_GAP;
  }
  const bottom = Math.max(y, hasWalk ? walkBottom : LANE_PAD);

  // Ports: a forward edge leaves a box's right side and enters the next one's left side, spread in
  // the order of their other ends; an arc or a band edge takes a side's lowest ports, or a lowest
  // box's bottom; a band box is reached by its top.
  const rights = new Sides();
  const lefts = new Sides();
  const bottoms = new Sides();
  const tops = new Sides();
  const middleY = (cell: Cell) => cell.y + cell.height / 2;
  const middleX = (cell: Cell) => cell.x + cell.width / 2;
  for (const laid of edges) {
    const { from, to } = laid;
    if (laid.route === "self" || (laid.direct && laid.route === "band")) {
      continue;
    }
    if (laid.route === "forward") {
      const next = laid.chain[1];
      const before = laid.chain[laid.chain.length - 2];
      rights.add(from, laid.id, next === undefined ? 0 : middleOf(next));
      lefts.add(to, laid.id, before === undefined ? 0 : middleOf(before));
    } else if (laid.direct) {
      (to.order > from.order ? bottoms : tops).add(from, laid.id, 0);
      (to.order > from.order ? tops : bottoms).add(to, laid.id, 0);
    } else if (laid.route === "beside") {
      rights.add(from, laid.id, middleY(to));
      rights.add(to, laid.id, middleY(from));
    } else if (laid.route === "arc") {
      if (lowest.has(from)) {
        bottoms.add(from, laid.id, laid.track);
      } else {
        rights.add(from, laid.id, BELOW_THE_WALK + laid.track);
      }
      if (lowest.has(to)) {
        bottoms.add(to, laid.id, -laid.track);
      } else {
        lefts.add(to, laid.id, BELOW_THE_WALK + laid.track);
      }
    } else if (from.level === null && to.level === null) {
      tops.add(from, laid.id, middleX(to));
      tops.add(to, laid.id, middleX(from));
    } else if (from.level === null) {
      tops.add(from, laid.bandKey, middleX(to));
      lefts.add(to, laid.id, 2 * BELOW_THE_WALK + laid.track);
    } else {
      rights.add(from, laid.id, 2 * BELOW_THE_WALK + laid.track);
      tops.add(to, laid.bandKey, middleX(from));
    }
  }
  const rightY = (cell: Cell, key: string) => middleY(cell) + rights.offset(cell, key, cell.height);
  const leftY = (cell: Cell, key: string) => middleY(cell) + lefts.offset(cell, key, cell.height);
  const bottomX = (cell: Cell, key: string) => middleX(cell) + bottoms.offset(cell, key, cell.width);
  const topX = (cell: Cell, key: string) => middleX(cell) + tops.offset(cell, key, cell.width);
  const gutterX = (cell: Cell) => cell.x - BAND_GAP / 2;

  const shaped = edges.map((laid): { laid: LaneEdge; path: string; points: Point[]; label: Point } => {
    const { from, to } = laid;
    if (laid.route === "self") {
      const loop = loopPath(from);
      return { laid, path: loop.path, points: loop.points, label: { x: loop.middle.x, y: loop.middle.y + LOOP_LABEL_DROP } };
    }
    if (laid.direct && laid.route === "band") {
      const leftward = to.order < from.order;
      const points = [
        { x: leftward ? from.x : from.x + from.width, y: middleY(from) },
        { x: leftward ? to.x + to.width : to.x, y: middleY(to) },
      ];
      return { laid, path: roundedPath(points), points, label: { x: ((points[0]?.x ?? 0) + (points[1]?.x ?? 0)) / 2, y: Math.min(from.y, to.y) - labelSize(laid.edge).height / 2 - 2 } };
    }
    if (laid.direct) {
      const downward = to.order > from.order;
      const points = [
        { x: (downward ? bottomX : topX)(from, laid.id), y: downward ? from.y + from.height : from.y },
        { x: (downward ? topX : bottomX)(to, laid.id), y: downward ? to.y : to.y + to.height },
      ];
      // Its words in the gap right of the column, clear of both boxes.
      const channel = rightOf(from);
      return { laid, path: roundedPath(points), points, label: { x: (channel.left + channel.right) / 2, y: ((points[0]?.y ?? 0) + (points[1]?.y ?? 0)) / 2 } };
    }
    if (laid.route === "forward") {
      const runs = laid.chain.map((slot, index): Run => {
        if (index === 0) {
          const at = from.x + from.width;
          return { from: at, to: at, y: rightY(from, laid.id) };
        }
        if (index === laid.chain.length - 1) {
          return { from: to.x, to: to.x, y: leftY(to, laid.id) };
        }
        const left = columnX[(from.level ?? 0) + index] ?? 0;
        return { from: left, to: left + BOX_WIDTH, y: middleOf(slot) };
      });
      return { laid, path: curvePath(runs), points: curvePoints(runs), label: curveMiddle(runs) };
    }
    if (laid.route === "beside") {
      const gx = rightOf(from).outX(laid.id);
      const fromY = rightY(from, laid.id);
      const toY = rightY(to, laid.id);
      const points = [
        { x: from.x + from.width, y: fromY },
        { x: gx, y: fromY },
        { x: gx, y: toY },
        { x: to.x + to.width, y: toY },
      ];
      const channel = rightOf(from);
      return { laid, path: roundedPath(points), points, label: { x: (channel.left + channel.right) / 2, y: (fromY + toY) / 2 } };
    }
    if (laid.route === "arc") {
      const track = trackY(laid.track);
      const out: Point[] = lowest.has(from)
        ? [
            { x: bottomX(from, laid.id), y: from.y + from.height },
            { x: bottomX(from, laid.id), y: track },
          ]
        : [
            { x: from.x + from.width, y: rightY(from, laid.id) },
            { x: rightOf(from).outX(`${laid.id}:out`), y: rightY(from, laid.id) },
            { x: rightOf(from).outX(`${laid.id}:out`), y: track },
          ];
      const into: Point[] = lowest.has(to)
        ? [
            { x: bottomX(to, laid.id), y: track },
            { x: bottomX(to, laid.id), y: to.y + to.height },
          ]
        : [
            { x: leftOf(to).inX(`${laid.id}:in`), y: track },
            { x: leftOf(to).inX(`${laid.id}:in`), y: leftY(to, laid.id) },
            { x: to.x, y: leftY(to, laid.id) },
          ];
      const points = [...out, ...into];
      return { laid, path: roundedPath(points), points, label: { x: ((out.at(-1)?.x ?? 0) + (into[0]?.x ?? 0)) / 2, y: track } };
    }
    // Band to band: up into the shelf's gap, along it, down its target's gutter when the shelf changes, into the top.
    if (from.level === null && to.level === null) {
      const outAt = shelfGap(from).y(`${laid.id}:out`);
      const inAt = shelfGap(to).y(`${laid.id}:in`);
      const startX = topX(from, laid.id);
      const endX = topX(to, laid.id);
      const points =
        from.shelf === to.shelf
          ? [
              { x: startX, y: from.y },
              { x: startX, y: outAt },
              { x: endX, y: outAt },
              { x: endX, y: to.y },
            ]
          : [
              { x: startX, y: from.y },
              { x: startX, y: outAt },
              { x: gutterX(to), y: outAt },
              { x: gutterX(to), y: inAt },
              { x: endX, y: inAt },
              { x: endX, y: to.y },
            ];
      return { laid, path: roundedPath(points), points, label: { x: ((points[1]?.x ?? 0) + (points[2]?.x ?? 0)) / 2, y: outAt } };
    }
    // A walk end and a band end: the walk box's side, its gap, the trunk over the band, the band box's gutter, its shelf's gap, its top.
    const trunk = trunkY(laid.track);
    const bandEnd = from.level === null ? from : to;
    const shelfAt = shelfGap(bandEnd).y(laid.bandKey);
    const bandSide: Point[] = [
      { x: gutterX(bandEnd), y: trunk },
      { x: gutterX(bandEnd), y: shelfAt },
      { x: topX(bandEnd, laid.bandKey), y: shelfAt },
      { x: topX(bandEnd, laid.bandKey), y: bandEnd.y },
    ];
    if (from.level !== null) {
      const gx = rightOf(from).outX(`${laid.id}:out`);
      const points = [{ x: from.x + from.width, y: rightY(from, laid.id) }, { x: gx, y: rightY(from, laid.id) }, { x: gx, y: trunk }, ...bandSide];
      return { laid, path: roundedPath(points), points, label: { x: (gx + gutterX(bandEnd)) / 2, y: trunk } };
    }
    const gx = leftOf(to).inX(`${laid.id}:in`);
    const points = [...[...bandSide].reverse(), { x: gx, y: trunk }, { x: gx, y: leftY(to, laid.id) }, { x: to.x, y: leftY(to, laid.id) }];
    return { laid, path: roundedPath(points), points, label: { x: (gx + gutterX(bandEnd)) / 2, y: trunk } };
  });

  spreadLabels(shaped);

  const dividers: FlowDivider[] = dividerColumns.map(({ level, stages }) => {
    const left = (columnX[level] ?? 0) - DIVIDER_INSET;
    return { id: `d:${String(level)}`, level, stages, x: left, y: LANE_PAD, width: BOX_WIDTH + DIVIDER_INSET, height: ruleBottom - LANE_PAD };
  });
  const right = Math.max(x, ...inBand.map((cell) => cell.x + cell.width + LANE_PAD));
  const width = Math.max(right, walkLeft + BOX_WIDTH + LANE_PAD, band.length > 0 ? walkLeft + BAND_CAPTION_WIDTH + LANE_PAD : 0);
  const height = bottom + LANE_PAD;
  return {
    boxes: cells.map((cell) => ({ id: cell.id, name: cell.name, node: cell.node, level: cell.level, order: cell.order, shelf: cell.shelf, x: cell.x, y: cell.y, width: cell.width, height: cell.height })),
    lanes: [
      {
        x: 0,
        y: 0,
        width,
        height,
        columns: walkColumns.map((column) => column.map((cell) => cell.id)),
        band: band.map((shelf) => shelf.map((cell) => cell.id)),
        bandY,
        dividers,
      },
    ],
    edges: shaped.map(({ laid, path, points, label }) => ({
      id: laid.id,
      edge: laid.edge,
      source: laid.from.id,
      target: laid.to.id,
      route: laid.route,
      offWalk: laid.from.node.distance === null || laid.to.node.distance === null,
      points,
      path,
      label,
    })),
    width,
    height,
    total,
    notDrawn: total - drawn.length,
  };
}

/**
 * How much room an edge's words take, its border and padding in: a word wraps within the label's
 * width, two lines at most, then an ellipsis (app.css `.flow-edge-label`). None: no label.
 */
export function labelSize(edge: FlowEdge): { width: number; height: number } {
  const lengths = edgeWords(edge).map((word) => Array.from(word).length * LABEL_CHARACTER);
  if (lengths.length === 0) {
    return { width: 0, height: 0 };
  }
  const lines = Math.min(
    LABEL_LINES,
    lengths.reduce((sum, length) => sum + Math.max(1, Math.ceil(length / (LABEL_WIDTH - 12))), 0),
  );
  return { width: Math.min(LABEL_WIDTH, Math.max(...lengths) + 12), height: lines * LABEL_LINE + 6 };
}

/**
 * Moves a label off the ones before it, so two edges between one pair of boxes say both: down a gap
 * between columns, along a track or a trunk below the walk.
 */
function spreadLabels(edges: readonly { laid: LaneEdge; label: Point }[]): void {
  const placed: Rect[] = [];
  for (const { laid, label } of edges) {
    const { width, height } = labelSize(laid.edge);
    if (height === 0) {
      continue;
    }
    const along = laid.route === "arc" || (laid.route === "band" && !laid.direct);
    for (let hit = true; hit; ) {
      hit = false;
      for (const other of placed) {
        if (Math.abs(other.x - label.x) * 2 < other.width + width && Math.abs(other.y - label.y) * 2 < other.height + height) {
          if (along) {
            label.x = other.x + (other.width + width) / 2 + 2;
          } else {
            label.y = other.y + (other.height + height) / 2 + 2;
          }
          hit = true;
        }
      }
    }
    placed.push({ x: label.x, y: label.y, width, height });
  }
}

/** The box among `ids` whose middle along `along` is nearest `at`; the first on a tie. */
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
 * The box a key moves to from `from`, or null to stay (docs/features/handoff-progress.md
 * "Acceptance criteria", F-5). On the walk, Right and Left go to the nearest box, by vertical centre,
 * of the next or previous column; Up and Down move within the column, Down from its last box to the
 * band's first shelf. In the band, Left and Right move along a shelf, Up and Down cross shelves, Up
 * from the first shelf to the last box of the nearest column.
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
  if (box.level !== null) {
    const column = lane.columns[box.level] ?? [];
    const at = column.indexOf(from);
    switch (key) {
      case "ArrowUp":
        return column[at - 1] ?? null;
      case "ArrowDown":
        return column[at + 1] ?? nearestBy(lane.band[0] ?? [], boxes, midX(box), midX);
      case "ArrowLeft":
        return nearestBy(lane.columns[box.level - 1] ?? [], boxes, midY(box), midY);
      case "ArrowRight":
        return nearestBy(lane.columns[box.level + 1] ?? [], boxes, midY(box), midY);
      default:
        return null;
    }
  }
  const shelf = lane.band[box.shelf] ?? [];
  const at = shelf.indexOf(from);
  switch (key) {
    case "ArrowLeft":
      return shelf[at - 1] ?? null;
    case "ArrowRight":
      return shelf[at + 1] ?? null;
    case "ArrowUp":
      return nearestBy(lane.band[box.shelf - 1] ?? lane.columns.flatMap((ids) => ids.slice(-1)), boxes, midX(box), midX);
    case "ArrowDown":
      return nearestBy(lane.band[box.shelf + 1] ?? [], boxes, midX(box), midX);
    default:
      return null;
  }
}
