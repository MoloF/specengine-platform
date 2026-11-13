import type { RoadmapDocument, RoadmapGroup, RoadmapItem } from "../api/types";
import { BOX_HEIGHT, BOX_WIDTH, CANVAS_LIMIT, COLUMN_STEP, ROW_STEP } from "../graph/layout";

// The chain canvas's placement (docs/features/roadmap-screen.md): no layout is computed. Each group
// is a column in the served order, empty releases too; each listed item a box at (its group's
// column, its index in the group), on the Graph's grid (ui/src/graph/layout.ts: 240 x 72 px boxes,
// columns 336 px apart, rows 88 px apart). An edge per `waits_on` pair whose two ends are drawn,
// from the predecessor's box to the item's; a pair naming the item itself is no edge (its "In a
// cycle" mark carries it). At most CANVAS_LIMIT boxes: whole lanes, left to right, until the next
// would pass it; then nothing more.

/**
 * The task cells a box draws on its one meta line, each at least its icon and ID; the rest are
 * counted after them, never wrapped onto a line the box has no room for (BOX_HEIGHT holds three:
 * docs/features/design-sweep-b.md "Acceptance criteria", DB3). The Lanes tab lists every one.
 */
export const CHAIN_TASKS_SHOWN = 2;

/** Where the first row of boxes starts: room above for the lane's name, marks and counts. */
export const LANE_HEAD = 96;

/**
 * The cut label's height: one line of `.chain-cut-label`'s line height, `--line-height-small: 1rem`
 * (tokens.css), 16 px at the browser's default root size. That token changing changes this number too.
 */
export const CUT_LABEL_HEIGHT = 16;

/** Between the cut label's foot and the lane heads' top; it also holds the label clear under a root size up to 24 px. */
export const CUT_LABEL_GAP = 8;

/** A box's canvas id: `i:` and the item's ID. */
export function boxId(item: string): string {
  return `i:${item}`;
}

export interface ChainBox {
  id: string;
  item: RoadmapItem;
  column: number;
  row: number;
  x: number;
  y: number;
}

export interface ChainLane {
  id: string;
  group: RoadmapGroup;
  column: number;
  x: number;
}

export interface ChainEdge {
  id: string;
  /** The predecessor's box. */
  source: string;
  /** The waiting item's box. */
  target: string;
  /** The predecessor's ID. */
  from: string;
  /** The waiting item's ID. */
  to: string;
  /** Into the next column: right side to left side; any other: an arc over the tops. */
  sourceHandle: "out" | "out-top";
  targetHandle: "in" | "in-top";
}

/** Set when whole lanes were left off the canvas: how many listed items, of how many. */
export interface ChainLimit {
  hidden: number;
  total: number;
}

export interface ChainLayout {
  /** The drawn lanes, left to right. */
  lanes: ChainLane[];
  /** The drawn boxes, lane by lane, top to bottom. */
  boxes: ChainBox[];
  edges: ChainEdge[];
  /** The x of the cut's rule, after the `mvp_cut` release's lane; null when no cut or its lane is not drawn. */
  cutX: number | null;
  /**
   * Where the cut's words start, at the rule's x: above every lane head, its foot CUT_LABEL_GAP over
   * y 0, so no lane's name, marks or counts in [0, LANE_HEAD) runs under it, however long the next
   * lane's title (docs/features/release-hardening.md "Acceptance criteria", RH-U6); null with no rule.
   */
  cutLabel: { x: number; y: number } | null;
  /** The height of the drawn lanes, the rule's length. */
  height: number;
  limit: ChainLimit | null;
}

/** Places the plan's listed items as served; pure. */
export function chainOf(plan: Pick<RoadmapDocument, "groups" | "cut">): ChainLayout {
  const total = plan.groups.reduce((count, group) => count + group.items.length, 0);
  const lanes: ChainLane[] = [];
  const boxes: ChainBox[] = [];
  let drawnItems = 0;
  let cut = false;
  let rows = 0;
  for (const [column, group] of plan.groups.entries()) {
    if (drawnItems + group.items.length > CANVAS_LIMIT) {
      cut = true;
      break;
    }
    drawnItems += group.items.length;
    const x = COLUMN_STEP * column;
    lanes.push({ id: `l:${String(column)}`, group, column, x });
    rows = Math.max(rows, group.items.length);
    group.items.forEach((item, row) => {
      boxes.push({ id: boxId(item.id), item, column, row, x, y: LANE_HEAD + ROW_STEP * row });
    });
  }
  const columnOf = new Map(boxes.map((box) => [box.item.id, box.column]));
  const edges: ChainEdge[] = [];
  const seen = new Set<string>();
  for (const box of boxes) {
    for (const from of box.item.waits_on) {
      const fromColumn = columnOf.get(from);
      const id = `w:${from}>${box.item.id}`;
      if (from === box.item.id || fromColumn === undefined || seen.has(id)) {
        continue;
      }
      seen.add(id);
      const next = box.column === fromColumn + 1;
      edges.push({
        id,
        source: boxId(from),
        target: box.id,
        from,
        to: box.item.id,
        sourceHandle: next ? "out" : "out-top",
        targetHandle: next ? "in" : "in-top",
      });
    }
  }
  const cutLane = plan.cut === null ? undefined : lanes.find((lane) => lane.group.mvp_cut);
  const cutX = cutLane === undefined ? null : cutLane.x + BOX_WIDTH + (COLUMN_STEP - BOX_WIDTH) / 2;
  return {
    lanes,
    boxes,
    edges,
    cutX,
    cutLabel: cutX === null ? null : { x: cutX, y: -(CUT_LABEL_HEIGHT + CUT_LABEL_GAP) },
    height: LANE_HEAD + ROW_STEP * Math.max(rows, 1) - (ROW_STEP - BOX_HEIGHT),
    limit: cut ? { hidden: total - drawnItems, total } : null,
  };
}
