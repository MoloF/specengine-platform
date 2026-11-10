import type { FlowEdge, FlowNode, FlowView } from "../api/types";
import { CANVAS_LIMIT, compareText, nodeName } from "../graph/layout";
import { BOX_HEIGHT, BOX_WIDTH, curveMiddle, curvePath, curvePoints, roundedPath, sideLoopPath, swapped, swappedPath, LOOP_DEPTH, type Point, type Rect, type Run } from "./geometry";
import { edgeWords } from "./labels";

// The Flow's layout (docs/features/flow-canvas.md "Layout"; docs/features/flow-scenario.md D7, D9),
// hand-written and pure: one read in (docs/features/writer-studio.md D1: a group is entered, never
// opened in place), boxes, its lane, its stage dividers and edges at absolute positions out. The walk
// reads top to bottom: a box's row is its `distance` in the read, its place in the row comes from four
// barycentre sweeps; the boxes off the walk wrap in a band under the last row. Rows decide every
// edge's shape, never a type or a `kind`, so a loop nobody declared shows as well as one declared; an
// edge against the walk runs on its own track right of it. Nothing here follows, lifts or counts a
// link: each edge drawn is the read's, unchanged. The input is sorted first, so the same read in any
// order gives the same picture.

export { BOX_HEIGHT, BOX_WIDTH, CANVAS_LIMIT };

/** Between two boxes of a row. */
export const BOX_GAP = 32;
/** The band between two rows at least: room for a forward edge's curve and its words. */
export const ROW_GAP = 56;
/** Inside a lane, each side. */
export const LANE_PAD = 16;
/** From one arc's track to the next, and one trunk's. */
export const TRACK_STEP = 12;
/** From one horizontal stretch to the next in a gap. */
export const SLOT_STEP = 8;
/** From one port to the next on a box's side. */
export const PORT_STEP = 8;
/** A stage divider's height, its words and its rule, above the row it opens. */
export const DIVIDER_HEIGHT = 24;
/** The band caption's least width: a picture with a band is at least this wide past the walk's left, so the caption stays inside it. */
export const BAND_CAPTION_WIDTH = 320;

/** What a long forward edge reserves in a row it passes, and its distance to what is beside it. */
const PASSAGE_WIDTH = 2;
const PASSAGE_GAP = 12;
/** Kept free beside a box with an edge to itself: the loop and its words. */
const LOOP_LABEL_SHIFT = 20;
const LOOP_ROOM = LOOP_DEPTH + LOOP_LABEL_SHIFT + 40;
/** From the walk's right edge to its first track; from a gap's edge to its first stretch. */
const ARC_PAD = 24;
const GAP_INSET = 12;
/** The band's caption line, the space between its boxes and its shelves, its columns at least. */
const BAND_HEAD = 40;
const BAND_GAP = 20;
const BAND_SHELF_GAP = 24;
const BAND_MIN_COLUMNS = 4;
/** Passes that align each row with its neighbours once the order is set. */
const PLACE_PASSES = 8;
/** An edge label's width at most, its line height, and a character's mean width, for telling two apart. */
const LABEL_WIDTH = 128;
const LABEL_LINE = 14;
const LABEL_CHARACTER = 6;
/** Room a gap keeps for a label stack between two rows. */
const LABEL_ROOM = 2 * LABEL_LINE + 8;

export interface FlowBox extends Rect {
  /** `n:` and the name, the same box in every layout; a second holder of the name adds its path and line. */
  id: string;
  name: string;
  node: FlowNode;
  /** Its row on the walk; null in the band under it. */
  level: number | null;
  /** Its place in its row, or in its band shelf, left to right. */
  order: number;
  /** Its band shelf; 0 on the walk. */
  shelf: number;
}

/** A stage divider: full width above the row where its stages begin, words raw (flow-scenario D7). */
export interface FlowDivider {
  id: string;
  level: number;
  /** The stages that begin in this row, left to right. */
  stages: string[];
  y: number;
}

export interface FlowLane extends Rect {
  /** Each walk row's box ids, left to right. */
  rows: string[][];
  /** The band's shelves of box ids, left to right. */
  band: string[][];
  /** Where the band's caption sits; null with no band. */
  bandY: number | null;
  dividers: FlowDivider[];
}

/** Forward down the walk, against it, along a row, to itself, or to or from the band. */
export type EdgeRoute = "forward" | "arc" | "beside" | "self" | "band";

export interface LaidEdge {
  id: string;
  /** The read's edge, unchanged. */
  edge: FlowEdge;
  source: string;
  target: string;
  route: EdgeRoute;
  /** Either end off the walk: drawn only while an end, or the edge, is selected (flow-scenario D9). */
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

/** A place in a row: a box, or the passage of a long forward edge. */
interface Slot {
  cell: Cell | null;
  width: number;
  rank: number;
  order: number;
  x: number;
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
  /** Its track right of the walk (an arc), or its trunk left of it (a band edge passing rows); -1 for none. */
  track: number;
  /** An arc whose two ends stand rightmost in their rows: it leaves and enters by their right sides. */
  side: boolean;
  /** Two boxes of a row or a shelf next to each other: a line from side to side. */
  direct: boolean;
}

/** A barycentre as a fraction, compared without rounding. */
interface Key {
  sum: number;
  count: number;
}

function compareKeys(a: Key, b: Key): number {
  return a.sum * b.count - b.sum * a.count;
}

/** Reorders a row by the mean place of each slot's neighbours; a slot with none keeps its place. */
function sweepRow(slots: readonly Slot[], near: (slot: Slot) => readonly Slot[]): Slot[] {
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
 * Lefts as near as can be to `wanted`, the order kept, `gaps[i]` between items i and i + 1:
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
  const lefts: number[] = [];
  for (const pool of pools) {
    for (let index = 0; index < pool.count; index += 1) {
      lefts.push(pool.sum / pool.count + (offsets[lefts.length] ?? 0));
    }
  }
  return lefts;
}

function centreOf(slot: Slot): number {
  return slot.x + (slot.cell === null ? slot.width : (slot.cell.width)) / 2;
}

/** Aligns one row with its neighbours on `near`; a slot with none stays where it is. */
function placeRow(slots: readonly Slot[], near: (slot: Slot) => readonly Slot[]): void {
  const wanted = slots.map((slot) => {
    const others = near(slot);
    const own = slot.cell === null ? slot.width : slot.cell.width;
    return others.length === 0 ? slot.x : others.reduce((sum, other) => sum + centreOf(other), 0) / others.length - own / 2;
  });
  const gaps = slots.map((slot, index) => {
    const next = slots[index + 1];
    return slot.cell === null || next?.cell === null ? PASSAGE_GAP : BOX_GAP;
  });
  const lefts = settle(
    wanted,
    slots.map((slot) => slot.width),
    gaps,
  );
  slots.forEach((slot, index) => {
    slot.x = lefts[index] ?? slot.x;
  });
}

function linked(a: Slot, b: Slot): void {
  a.after.push(b);
  b.before.push(a);
}

function slotOf(cell: Cell, rank: number): Slot {
  return { cell, width: cell.width + (cell.loops ? LOOP_ROOM : 0), rank, order: rank, x: 0, before: [], after: [] };
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

/** The horizontal stretches of one gap, each its own height, in the order asked. */
class Gap {
  private readonly slots = new Map<string, number>();
  top = 0;

  ask(key: string): void {
    if (!this.slots.has(key)) {
      this.slots.set(key, this.slots.size);
    }
  }

  get size(): number {
    return this.slots.size;
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

/** A forward edge's vertical runs, drawn as the horizontal form with its axes swapped. */
function downRuns(runs: readonly { from: number; to: number; x: number }[]): Run[] {
  return runs.map((run) => ({ from: run.from, to: run.to, y: run.x }));
}

/** The stage dividers of the walk's rows: a stage's divider stands above the first row holding a box of it; fewer than two stages, none. */
function dividersOf(rows: readonly Cell[][]): { level: number; stages: string[] }[] {
  const begun = new Map<string, number>();
  rows.forEach((row, level) => {
    for (const cell of row) {
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
    return [{ edge, id: edgeKey(edge), from, to, route: routeOf(from, to), chain: [], track: -1, side: false, direct: false }];
  });
  for (const laid of edges) {
    if (laid.route === "self") {
      laid.from.loops = true;
    }
  }

  // Rows, a passage for each row a forward edge skips; four sweeps.
  const rows: Slot[][] = layers.map(() => []);
  const slotByCell = new Map<Cell, Slot>();
  let rank = 0;
  for (const cell of cells) {
    if (cell.level !== null) {
      const slot = slotOf(cell, rank);
      rank += 1;
      slotByCell.set(cell, slot);
      rows[cell.level]?.push(slot);
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
      const passage: Slot = { cell: null, width: PASSAGE_WIDTH, rank, order: rank, x: 0, before: [], after: [] };
      rank += 1;
      rows[level]?.push(passage);
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
  for (const slots of rows) {
    slots.forEach((slot, order) => {
      slot.order = order;
    });
  }
  const orders = rows.map((slots) => slots);
  const sweep = (down: boolean) => {
    const sequence = orders.map((_, index) => (down ? index : orders.length - 1 - index)).slice(1);
    for (const at of sequence) {
      orders[at] = sweepRow(orders[at] ?? [], (slot) => (down ? slot.before : slot.after));
    }
  };
  sweep(true);
  sweep(false);
  sweep(true);
  sweep(false);

  // Each row laid left to right, then pulled towards its neighbours: a join sits between its branches.
  for (const slots of orders) {
    let left = 0;
    slots.forEach((slot, index) => {
      slot.x = left;
      const next = slots[index + 1];
      left += slot.width + (slot.cell === null || next?.cell === null ? PASSAGE_GAP : BOX_GAP);
    });
  }
  for (let pass = 0; pass < PLACE_PASSES; pass += 1) {
    const down = pass % 2 === 0;
    const sequence = orders.map((_, index) => (down ? index : orders.length - 1 - index));
    for (const at of sequence) {
      placeRow(orders[at] ?? [], (slot) => (down ? slot.before : slot.after));
    }
  }

  // The band: its boxes under the walk boxes they link to, wrapped at the walk's width or four boxes.
  const inBand = cells.filter((cell) => cell.level === null);
  const bandEdges = edges.filter((laid) => laid.route === "band");
  // A band edge passes the rows on a trunk left of them when its walk end stands above the last row,
  // or when it climbs from the band into the walk; down from the last row it drops in the band's gutter.
  const trunked = (laid: LaneEdge) => {
    if (laid.from.level === null) {
      return laid.to.level !== null;
    }
    return laid.to.level === null && laid.from.level < layers.length - 1;
  };
  const trunks = new Map<string, number>();
  for (const laid of bandEdges.filter(trunked)) {
    const walk = laid.from.level === null ? laid.to : laid.from;
    const key = `${walk.id}:${laid.edge.type}:${laid.edge.kind ?? ""}`;
    const known = trunks.get(key);
    if (known === undefined) {
      trunks.set(key, trunks.size);
    }
    laid.track = trunks.get(key) ?? 0;
  }
  const trunkRoom = trunks.size === 0 ? 0 : GAP_INSET + trunks.size * TRACK_STEP;
  const lowest = Math.min(0, ...orders.flatMap((slots) => slots.map((slot) => slot.x)));
  const walkLeft = LANE_PAD + trunkRoom;
  let walkRight = walkLeft;
  orders.forEach((slots, level) => {
    let order = 0;
    for (const slot of slots) {
      slot.x = Math.round(slot.x - lowest) + walkLeft;
      walkRight = Math.max(walkRight, slot.x + slot.width);
      if (slot.cell !== null) {
        slot.cell.x = slot.x;
        slot.cell.level = level;
        slot.cell.order = order;
        order += 1;
      }
    }
  });
  const walkRows = orders.map((slots) => slots.flatMap((slot) => (slot.cell === null ? [] : [slot.cell])));
  const walkWidth = walkRows.length === 0 ? 0 : walkRight - walkLeft;
  const fitting = Math.floor((walkWidth + BAND_GAP) / (BOX_WIDTH + BAND_GAP));
  const bandColumns = Math.max(1, Math.min(inBand.length, Math.max(fitting, BAND_MIN_COLUMNS)));
  const bandOrder = bandOrderOf(inBand, edges);
  const band: Cell[][] = [];
  bandOrder.forEach((cell, index) => {
    const shelf = Math.floor(index / bandColumns);
    cell.shelf = shelf;
    cell.order = index % bandColumns;
    cell.column = index % bandColumns;
    cell.x = walkLeft + cell.column * (BOX_WIDTH + BAND_GAP);
    const row = band[shelf] ?? [];
    row.push(cell);
    band[shelf] = row;
  });

  // Arcs right of the walk: the shortest nearest it, so longer ones wrap them; an arc whose two ends
  // stand rightmost in their rows leaves and enters by their right sides.
  const rightmost = new Set(walkRows.flatMap((row) => (row.length === 0 ? [] : [row[row.length - 1]])));
  const span = (laid: LaneEdge) => Math.abs((laid.from.level ?? 0) - (laid.to.level ?? 0));
  const arcs = edges.filter((laid) => laid.route === "arc").sort((a, b) => span(a) - span(b) || compareText(a.id, b.id));
  arcs.forEach((laid, track) => {
    laid.track = track;
    laid.side = rightmost.has(laid.from) && rightmost.has(laid.to);
  });
  const trackX = (track: number) => walkRight + ARC_PAD + track * TRACK_STEP;
  const trunkX = (track: number) => LANE_PAD + GAP_INSET + (trunks.size - 1 - track) * TRACK_STEP;

  // Next to each other in a row or a shelf: a line from side to side.
  for (const laid of edges) {
    const neighbours = laid.from.level === laid.to.level && laid.from.shelf === laid.to.shelf && Math.abs(laid.from.order - laid.to.order) === 1;
    if ((laid.route === "beside" || (laid.route === "band" && laid.from.level === null && laid.to.level === null)) && neighbours) {
      laid.direct = true;
    }
  }

  // The gaps: above each row (-1 above the first), and above each band shelf. Each stretch asks its own height.
  const gaps = Array.from({ length: layers.length + 1 }, () => new Gap());
  const above = (level: number) => gaps[level] ?? new Gap();
  const below = (level: number) => gaps[level + 1] ?? new Gap();
  const shelfGaps = Array.from({ length: band.length }, () => new Gap());
  const shelfGap = (shelf: number) => shelfGaps[shelf] ?? new Gap();
  for (const laid of edges) {
    const { from, to } = laid;
    if (laid.direct || laid.route === "self" || laid.route === "forward") {
      continue;
    }
    if (laid.route === "beside" && from.level !== null) {
      below(from.level).ask(laid.id);
    } else if (laid.route === "arc" && !laid.side && from.level !== null && to.level !== null) {
      below(from.level).ask(`${laid.id}:out`);
      above(to.level).ask(`${laid.id}:in`);
    } else if (laid.route === "band") {
      if (from.level !== null) {
        below(from.level).ask(`${laid.id}:out`);
      } else {
        shelfGap(from.shelf).ask(`${laid.id}:out`);
      }
      if (to.level !== null) {
        above(to.level).ask(`${laid.id}:in`);
      } else {
        shelfGap(to.shelf).ask(`${laid.id}:in`);
      }
    }
  }

  // Heights, top to bottom: each row's gap above it, its divider, the row; then the band.
  const dividerRows = dividersOf(walkRows);
  const dividerAt = new Map(dividerRows.map((divider) => [divider.level, divider]));
  const labelled = new Set(edges.filter((laid) => laid.route === "forward" && edgeWords(laid.edge).length > 0).map((laid) => laid.to.level));
  let y = LANE_PAD;
  const rowTop: number[] = [];
  const dividers: FlowDivider[] = [];
  layers.forEach((_, level) => {
    const gap = above(level);
    gap.top = y;
    const room = level === 0 ? gap.room : Math.max(ROW_GAP, gap.room + (labelled.has(level) ? LABEL_ROOM : GAP_INSET));
    y += room;
    const divider = dividerAt.get(level);
    if (divider !== undefined) {
      dividers.push({ id: `d:${String(level)}`, level, stages: divider.stages, y });
      y += DIVIDER_HEIGHT;
    }
    rowTop.push(y);
    for (const cell of walkRows[level] ?? []) {
      cell.y = y;
    }
    y += BOX_HEIGHT;
  });
  const last = below(layers.length - 1);
  last.top = y;
  if (layers.length > 0) {
    y += last.room + (band.length > 0 || last.size > 0 ? GAP_INSET : 0);
  }
  let bandY: number | null = null;
  if (band.length > 0) {
    bandY = y + (layers.length > 0 ? 8 : 0);
    y = bandY + BAND_HEAD;
    band.forEach((shelf, index) => {
      const gap = shelfGap(index);
      gap.top = y - (index === 0 ? 0 : BAND_SHELF_GAP);
      y = Math.max(y, gap.top + gap.room);
      const tallest = Math.max(...shelf.map((cell) => cell.height));
      for (const cell of shelf) {
        cell.y = y;
      }
      y += tallest + BAND_SHELF_GAP;
    });
    y -= BAND_SHELF_GAP;
  }
  const bottom = y;

  // Ports: a walk box's edges leave its bottom and enter its top, spread in the order of their other ends; a band box's both by its top.
  const tops = new Sides();
  const bottoms = new Sides();
  const rights = new Sides();
  const middleX = (cell: Cell) => cell.x + cell.width / 2;
  for (const laid of edges) {
    const { from, to } = laid;
    if (laid.direct || laid.route === "self") {
      continue;
    }
    if (laid.route === "forward") {
      const next = laid.chain[1];
      const before = laid.chain[laid.chain.length - 2];
      bottoms.add(from, laid.id, next === undefined ? 0 : centreOf(next));
      tops.add(to, laid.id, before === undefined ? 0 : centreOf(before));
    } else if (laid.route === "beside") {
      bottoms.add(from, laid.id, middleX(to));
      bottoms.add(to, laid.id, middleX(from));
    } else if (laid.route === "arc" && laid.side) {
      rights.add(from, laid.id, -laid.track);
      rights.add(to, laid.id, -laid.track);
    } else {
      (from.level === null ? tops : bottoms).add(from, laid.id, middleX(to));
      tops.add(to, laid.id, middleX(from));
    }
  }
  const outX = (cell: Cell, key: string) => middleX(cell) + (cell.level === null ? tops : bottoms).offset(cell, key, cell.width);
  const outY = (cell: Cell) => (cell.level === null ? cell.y : cell.y + cell.height);
  const inX = (cell: Cell, key: string) => middleX(cell) + tops.offset(cell, key, cell.width);
  const rightY = (cell: Cell, key: string) => cell.y + cell.height / 2 + rights.offset(cell, key, cell.height);
  const gapOut = (cell: Cell, key: string) => (cell.level === null ? shelfGap(cell.shelf) : below(cell.level)).y(key);
  const gapIn = (cell: Cell, key: string) => (cell.level === null ? shelfGap(cell.shelf) : above(cell.level)).y(key);
  const gutterX = (cell: Cell) => cell.x - BAND_GAP / 2;

  const shaped = edges.map((laid): { laid: LaneEdge; path: string; points: Point[]; label: Point } => {
    const { from, to } = laid;
    if (laid.route === "self") {
      const loop = sideLoopPath(from);
      return { laid, path: loop.path, points: loop.points, label: { x: loop.middle.x + LOOP_LABEL_SHIFT, y: loop.middle.y } };
    }
    if (laid.direct) {
      const leftward = to.order < from.order;
      const midY = (cell: Cell) => cell.y + cell.height / 2;
      const points = [
        { x: leftward ? from.x : from.x + from.width, y: midY(from) },
        { x: leftward ? to.x + to.width : to.x, y: midY(to) },
      ];
      return { laid, path: roundedPath(points), points, label: { x: ((points[0]?.x ?? 0) + (points[1]?.x ?? 0)) / 2, y: (points[0]?.y ?? 0) - LABEL_LINE } };
    }
    if (laid.route === "forward") {
      const runs = laid.chain.map((slot, index) => {
        if (index === 0) {
          return { from: outY(from), to: outY(from), x: outX(from, laid.id) };
        }
        if (index === laid.chain.length - 1) {
          return { from: to.y, to: to.y, x: inX(to, laid.id) };
        }
        const level = (from.level ?? 0) + index;
        const top = rowTop[level] ?? 0;
        return { from: top, to: top + BOX_HEIGHT, x: slot.x + slot.width / 2 };
      });
      const flat = downRuns(runs);
      return { laid, path: swappedPath(curvePath(flat)), points: curvePoints(flat).map(swapped), label: swapped(curveMiddle(flat)) };
    }
    if (laid.route === "beside") {
      const y = gapOut(from, laid.id);
      const points = [
        { x: outX(from, laid.id), y: outY(from) },
        { x: outX(from, laid.id), y },
        { x: outX(to, laid.id), y },
        { x: outX(to, laid.id), y: outY(to) },
      ];
      return { laid, path: roundedPath(points), points, label: { x: ((points[1]?.x ?? 0) + (points[2]?.x ?? 0)) / 2, y } };
    }
    if (laid.route === "arc") {
      const x = trackX(laid.track);
      if (laid.side) {
        const points = [
          { x: from.x + from.width, y: rightY(from, laid.id) },
          { x, y: rightY(from, laid.id) },
          { x, y: rightY(to, laid.id) },
          { x: to.x + to.width, y: rightY(to, laid.id) },
        ];
        return { laid, path: roundedPath(points), points, label: { x, y: ((points[1]?.y ?? 0) + (points[2]?.y ?? 0)) / 2 } };
      }
      const outKey = `${laid.id}:out`;
      const inKey = `${laid.id}:in`;
      const points = [
        { x: outX(from, laid.id), y: outY(from) },
        { x: outX(from, laid.id), y: gapOut(from, outKey) },
        { x, y: gapOut(from, outKey) },
        { x, y: gapIn(to, inKey) },
        { x: inX(to, laid.id), y: gapIn(to, inKey) },
        { x: inX(to, laid.id), y: to.y },
      ];
      return { laid, path: roundedPath(points), points, label: { x, y: ((points[2]?.y ?? 0) + (points[3]?.y ?? 0)) / 2 } };
    }
    // To or from the band: down the gap below the walk end, then a trunk or the band's gutter, into the far end's top.
    const outKey = `${laid.id}:out`;
    const inKey = `${laid.id}:in`;
    const startX = outX(from, laid.id);
    const endX = inX(to, laid.id);
    const outAt = gapOut(from, outKey);
    const inAt = gapIn(to, inKey);
    const bandEnd = to.level === null ? to : from;
    const down = laid.track >= 0 ? trunkX(laid.track) : gutterX(bandEnd);
    const sameGap = from.level === null && to.level === null && from.shelf === to.shelf;
    const points = sameGap
      ? [
          { x: startX, y: outY(from) },
          { x: startX, y: outAt },
          { x: endX, y: outAt },
          { x: endX, y: to.y },
        ]
      : [
          { x: startX, y: outY(from) },
          { x: startX, y: outAt },
          { x: down, y: outAt },
          { x: down, y: inAt },
          { x: endX, y: inAt },
          { x: endX, y: to.y },
        ];
    return { laid, path: roundedPath(points), points, label: { x: ((points[1]?.x ?? 0) + (points[2]?.x ?? 0)) / 2, y: outAt } };
  });

  spreadLabels(shaped);

  const right = Math.max(walkRight + (arcs.length > 0 ? ARC_PAD + arcs.length * TRACK_STEP : 0), ...inBand.map((cell) => cell.x + cell.width)) + LANE_PAD;
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
        rows: walkRows.map((row) => row.map((cell) => cell.id)),
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
          label.x = other.x + (other.width + width) / 2 + 2;
          hit = true;
        }
      }
    }
    placed.push({ x: label.x, y: label.y, width, height });
  }
}

function nearestBy(ids: readonly string[], boxes: ReadonlyMap<string, FlowBox>, at: number): string | null {
  let best: string | null = null;
  let distance = Infinity;
  for (const id of ids) {
    const box = boxes.get(id);
    if (box !== undefined && Math.abs(box.x + box.width / 2 - at) < distance) {
      best = id;
      distance = Math.abs(box.x + box.width / 2 - at);
    }
  }
  return best;
}

/**
 * The box a key moves to from `from`, or null to stay (flow-scenario "Keys"). Up and Down go across
 * the rows to the nearest box, the band last; Left and Right along a row, and along a band shelf.
 */
export function boxAfterKey(layout: FlowLayout, from: string, key: string): string | null {
  const boxes = new Map(layout.boxes.map((box) => [box.id, box]));
  const box = boxes.get(from);
  const lane = layout.lanes[0];
  if (box === undefined || lane === undefined) {
    return null;
  }
  const middle = box.x + box.width / 2;
  const row = box.level === null ? (lane.band[box.shelf] ?? []) : (lane.rows[box.level] ?? []);
  const at = row.indexOf(from);
  switch (key) {
    case "ArrowLeft":
      return row[at - 1] ?? null;
    case "ArrowRight":
      return row[at + 1] ?? null;
    case "ArrowUp": {
      if (box.level !== null) {
        return nearestBy(lane.rows[box.level - 1] ?? [], boxes, middle);
      }
      return nearestBy(lane.band[box.shelf - 1] ?? lane.rows[lane.rows.length - 1] ?? [], boxes, middle);
    }
    case "ArrowDown": {
      if (box.level !== null) {
        return nearestBy(lane.rows[box.level + 1] ?? lane.band[0] ?? [], boxes, middle);
      }
      return nearestBy(lane.band[box.shelf + 1] ?? [], boxes, middle);
    }
    default:
      return null;
  }
}
