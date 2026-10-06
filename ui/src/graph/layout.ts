import type { FollowedType, GraphEdge, GraphNode, GraphView } from "../api/types";

// The canvas's layered layout (docs/features/ui-graph.md "Layout"), hand-written and pure: one
// answer of `spec graph` in, boxes and edges out. Columns by distance, rows by four barycenter
// sweeps, ties by (path, line); the input is sorted first, so the same answer in any order gives
// the same picture. The walk itself stays in core: nothing here follows, unions or infers a link.

/** A box's size and the grid it sits on, in canvas pixels. */
export const BOX_WIDTH = 240;
export const BOX_HEIGHT = 72;
export const COLUMN_STEP = 336;
export const ROW_STEP = 88;
export const HANDLE_SIZE = 8;

/** Boxes a column keeps before the rest collapse into "+k more at distance d". */
export const COLUMN_LIMIT = 24;

/** Boxes the canvas draws at most; whole columns, left to right. */
export const CANVAS_LIMIT = 200;

/** The stroke patterns of the followed types, by position in the mode's chip order (cycling); never by name. */
export const PATTERNS = ["none", "10 5", "3 4", "14 4 3 4", "1 5", "18 6 6 6"] as const;

/** A type's stroke pattern from its position in the mode's chip order. */
export function patternAt(position: number): string {
  return PATTERNS[((position % PATTERNS.length) + PATTERNS.length) % PATTERNS.length] ?? "none";
}

export type SourceHandle = "out" | "out-top";
export type TargetHandle = "in" | "in-top";

/** A node's name: its ID, else its path (`docs/canon/spec-cli-graph.md` "spec graph"). */
export function nodeName(node: { id: string | null; path: string }): string {
  return node.id ?? node.path;
}

/** A name the walk reached, with each node holding it (by path, line) and its distance. */
export interface Walked {
  name: string;
  holders: GraphNode[];
  distance: number;
}

/** An edge end the walk did not reach: a nested section named by itself, or an unresolved end. */
export type Stub =
  | { type: "section"; name: string }
  | { type: "unresolved"; edge: GraphEdge; end: "src" | "dst" };

/** A stub and the edges ending at it, in the order given. */
export interface StubEnd {
  id: string;
  stub: Stub;
  edges: GraphEdge[];
}

/** What a box shows; a more-box counts the walked names among the boxes it hides (the rest are stubs). */
export type BoxContent =
  | { type: "walked"; walked: Walked }
  | { type: "stub"; stub: StubEnd }
  | { type: "more"; hidden: number; hiddenWalked: number; column: number };

export interface LayoutBox {
  /**
   * The canvas id, the same for the same box in every answer: `n:` a walked name, `s:` a section
   * stub, `u:` an unresolved end (by its end, type, path, line and written), `m:` a more-box.
   */
  id: string;
  content: BoxContent;
  column: number;
  row: number;
  x: number;
  y: number;
}

export interface LayoutEdge {
  id: string;
  edge: GraphEdge;
  /** The walk-from box: `src` when the type is followed out, else `dst`. */
  source: string;
  /** The walk-to box. */
  target: string;
  sourceHandle: SourceHandle;
  targetHandle: TargetHandle;
  /** Where the arrow goes: at `dst`, the walk-to end (`end`) or the walk-from end (`start`). */
  arrowAt: "end" | "start";
  /** The type's position in the mode's chip order; -1 when the order does not hold it. */
  position: number;
}

/** Set when whole columns were left off the canvas. */
export interface CanvasLimit {
  lastColumn: number;
  shown: number;
  total: number;
}

export interface Layout {
  /** Drawn boxes by (column, row). */
  boxes: LayoutBox[];
  /** Drawn edges: both ends drawn. */
  edges: LayoutEdge[];
  /** Drawn columns left to right, each its box ids top to bottom. */
  columns: { column: number; boxes: string[] }[];
  limit: CanvasLimit | null;
}

export function compareText(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

function byPlace(a: { path: string; line: number }, b: { path: string; line: number }): number {
  return compareText(a.path, b.path) || a.line - b.line;
}

function byNode(a: GraphNode, b: GraphNode): number {
  return a.distance - b.distance || byPlace(a, b) || compareText(nodeName(a), nodeName(b));
}

/** A total order of edges: the answer's (type, path, line, written), then every other field. */
function byEdge(a: GraphEdge, b: GraphEdge): number {
  return (
    compareText(a.type, b.type) ||
    byPlace(a, b) ||
    compareText(a.written, b.written) ||
    compareText(a.src ?? "", b.src ?? "") ||
    compareText(a.dst ?? "", b.dst ?? "") ||
    compareText(a.state, b.state) ||
    compareText(a.reason ?? "", b.reason ?? "")
  );
}

export interface Ends {
  /** The names reached, in the order of `nodes`. */
  walked: Map<string, Walked>;
  /** Every stub, by its first edge. */
  stubs: StubEnd[];
  /** Each edge's end boxes, by index into `edges`. */
  ends: { src: string; dst: string }[];
}

/** An unresolved end's id: from what the link is (end, type, path, line, written), never its index. */
function unresolvedId(edge: GraphEdge, end: "src" | "dst"): string {
  return `u:${end}:${encodeURIComponent(edge.type)}:${encodeURIComponent(edge.path)}:${String(edge.line)}:${encodeURIComponent(edge.written)}`;
}

/**
 * The names the walk reached and the stubs at the other edge ends: a section stub per end name
 * outside `nodes`, an unresolved stub per `null` end. Never an anchor, never expanded. A stub's id
 * names the same end in every answer, so a selection or a tab stop never lands on another one.
 */
export function endsOf(nodes: readonly GraphNode[], edges: readonly GraphEdge[]): Ends {
  const walked = new Map<string, Walked>();
  for (const node of nodes) {
    const name = nodeName(node);
    const known = walked.get(name);
    if (known === undefined) {
      walked.set(name, { name, holders: [node], distance: node.distance });
    } else {
      known.holders.push(node);
      known.distance = Math.min(known.distance, node.distance);
    }
  }
  for (const entry of walked.values()) {
    entry.holders.sort(byPlace);
  }
  const stubs: StubEnd[] = [];
  const sections = new Map<string, StubEnd>();
  const seen = new Map<string, number>();
  const ends = edges.map((edge) => {
    const endOf = (end: "src" | "dst"): string => {
      const name = edge[end];
      if (name === null) {
        // The same link written twice on one line: the later copies count on.
        const base = unresolvedId(edge, end);
        const copies = (seen.get(base) ?? 0) + 1;
        seen.set(base, copies);
        const stub: StubEnd = { id: copies === 1 ? base : `${base}#${String(copies)}`, stub: { type: "unresolved", edge, end }, edges: [edge] };
        stubs.push(stub);
        return stub.id;
      }
      if (walked.has(name)) {
        return `n:${name}`;
      }
      let stub = sections.get(name);
      if (stub === undefined) {
        stub = { id: `s:${name}`, stub: { type: "section", name }, edges: [] };
        sections.set(name, stub);
        stubs.push(stub);
      }
      // A link from a section to itself ends here twice: it is one edge of the stub.
      if (stub.edges[stub.edges.length - 1] !== edge) {
        stub.edges.push(edge);
      }
      return stub.id;
    };
    return { src: endOf("src"), dst: endOf("dst") };
  });
  return { walked, stubs, ends };
}

/** A barycenter key as a fraction, compared without rounding. */
interface Key {
  sum: number;
  count: number;
}

function compareKeys(a: Key, b: Key): number {
  return a.sum * b.count - b.sum * a.count;
}

/** The followed direction of a type in this answer: `in` walks from `dst`, anything else from `src`. */
function followsIn(types: readonly FollowedType[], type: string): boolean {
  return types.find((followed) => followed.type === type)?.direction === "in";
}

/** Each type's position in `order`; -1 for a type the order does not hold. */
function positionsIn(order: readonly string[]): (type: string) => number {
  const positions = new Map<string, number>();
  order.forEach((type, position) => {
    if (!positions.has(type)) {
      positions.set(type, position);
    }
  });
  return (type) => positions.get(type) ?? -1;
}

/**
 * Lays out one answer (docs/features/ui-graph.md "Layout", steps 1-6). Pure and deterministic: the
 * input is sorted first; boxes are fixed 240 x 72 px, column c at x = 336c, row r at y = 88r.
 * `typeOrder` is the mode's chip order, which styles each type; by default the answer's `types`.
 */
export function layoutGraph(
  view: Pick<GraphView, "types" | "nodes" | "edges">,
  typeOrder: readonly string[] = view.types.map((followed) => followed.type),
): Layout {
  const nodes = [...view.nodes].sort(byNode);
  const edges = [...view.edges].sort(byEdge);
  const { walked, stubs, ends } = endsOf(nodes, edges);
  const walkedId = (name: string) => `n:${name}`;
  const walkedById = new Map([...walked.values()].map((entry) => [walkedId(entry.name), entry]));

  // 1-2. Boxes and their columns. A stub sits beside the walked other end of its first edge that
  // has one: a section stub the edge is walked from one column before it (never below 0), any
  // other stub one column after it. Edge direction only: the UI infers no containment.
  const column = new Map<string, number>();
  for (const entry of walked.values()) {
    column.set(walkedId(entry.name), entry.distance);
  }
  const edgeIndex = new Map(edges.map((edge, index) => [edge, index]));
  const pending: string[] = [];
  for (const stub of stubs) {
    let placed: number | null = null;
    for (const edge of stub.edges) {
      const pair = ends[edgeIndex.get(edge) ?? -1];
      if (pair === undefined) {
        continue;
      }
      const far = walkedById.get(pair.src === stub.id ? pair.dst : pair.src);
      if (far !== undefined) {
        const walkFrom = followsIn(view.types, edge.type) ? pair.dst : pair.src;
        placed = stub.stub.type === "section" && walkFrom === stub.id ? Math.max(0, far.distance - 1) : far.distance + 1;
        break;
      }
    }
    if (placed === null) {
      pending.push(stub.id);
    } else {
      column.set(stub.id, placed);
    }
  }
  let lastColumn = -1;
  for (const at of column.values()) {
    lastColumn = Math.max(lastColumn, at);
  }
  for (const id of pending) {
    column.set(id, lastColumn + 1);
  }

  // 3. Collapse: walked by (path, line), then stubs by first edge; over 24, the rest is one box.
  const placeOf = (id: string): GraphNode | undefined => walkedById.get(id)?.holders[0];
  const byColumn = new Map<number, string[]>();
  const walkedIds = [...walkedById.keys()].sort((a, b) => {
    const left = placeOf(a);
    const right = placeOf(b);
    return (left !== undefined && right !== undefined ? byPlace(left, right) : 0) || compareText(a, b);
  });
  for (const id of [...walkedIds, ...stubs.map((stub) => stub.id)]) {
    const at = column.get(id) ?? 0;
    const ids = byColumn.get(at);
    if (ids === undefined) {
      byColumn.set(at, [id]);
    } else {
      ids.push(id);
    }
  }
  const more = new Map<number, { hidden: number; hiddenWalked: number }>();
  for (const [at, ids] of byColumn) {
    if (ids.length > COLUMN_LIMIT) {
      const walkedHere = ids.reduce((count, id) => count + (walkedById.has(id) ? 1 : 0), 0);
      more.set(at, { hidden: ids.length - COLUMN_LIMIT, hiddenWalked: Math.max(0, walkedHere - COLUMN_LIMIT) });
      byColumn.set(at, ids.slice(0, COLUMN_LIMIT));
    }
  }

  // 4. Limit: whole columns left to right while the total stays within 200 boxes.
  const drawnColumns: number[] = [];
  let total = 0;
  let cut = false;
  for (const at of [...byColumn.keys()].sort((a, b) => a - b)) {
    const size = (byColumn.get(at)?.length ?? 0) + (more.has(at) ? 1 : 0);
    if (total + size > CANVAS_LIMIT) {
      cut = true;
      break;
    }
    total += size;
    drawnColumns.push(at);
  }
  const drawn = new Set(drawnColumns.flatMap((at) => byColumn.get(at) ?? []));

  // 5. Order: walked by (path, line), then four sweeps by the mean row of walked neighbours in the
  // column before; stubs and the more-box follow, unswept.
  const neighbours = new Map<string, Set<string>>();
  for (const pair of ends) {
    if (pair.src !== pair.dst && walkedById.has(pair.src) && walkedById.has(pair.dst) && drawn.has(pair.src) && drawn.has(pair.dst)) {
      for (const [from, to] of [
        [pair.src, pair.dst],
        [pair.dst, pair.src],
      ] as const) {
        const set = neighbours.get(from) ?? new Set<string>();
        set.add(to);
        neighbours.set(from, set);
      }
    }
  }
  const order = new Map<number, string[]>();
  const row = new Map<string, number>();
  for (const at of drawnColumns) {
    const ids = byColumn.get(at) ?? [];
    const walkedHere = ids.filter((id) => walkedById.has(id));
    const rest = ids.filter((id) => !walkedById.has(id));
    order.set(at, [...walkedHere, ...rest]);
    walkedHere.forEach((id, index) => row.set(id, index));
    rest.forEach((id, index) => row.set(id, walkedHere.length + index));
  }
  const sweep = (down: boolean) => {
    const sequence = down ? drawnColumns : [...drawnColumns].reverse();
    for (const at of sequence.slice(1)) {
      const before = down ? at - 1 : at + 1;
      const ids = order.get(at) ?? [];
      const walkedHere = ids.filter((id) => walkedById.has(id));
      const rest = ids.filter((id) => !walkedById.has(id));
      const keys = new Map<string, Key>();
      for (const id of walkedHere) {
        const rows = [...(neighbours.get(id) ?? [])].filter((other) => column.get(other) === before).map((other) => row.get(other) ?? 0);
        keys.set(id, rows.length === 0 ? { sum: row.get(id) ?? 0, count: 1 } : { sum: rows.reduce((a, b) => a + b, 0), count: rows.length });
      }
      walkedHere.sort((a, b) => {
        const left = placeOf(a);
        const right = placeOf(b);
        return (
          compareKeys(keys.get(a) ?? { sum: 0, count: 1 }, keys.get(b) ?? { sum: 0, count: 1 }) ||
          (left !== undefined && right !== undefined ? byPlace(left, right) : 0) ||
          compareText(a, b)
        );
      });
      const next = [...walkedHere, ...rest];
      order.set(at, next);
      next.forEach((id, index) => row.set(id, index));
    }
  };
  sweep(true);
  sweep(false);
  sweep(true);
  sweep(false);

  // 6. Positions; the more-box ends its column.
  const boxes: LayoutBox[] = [];
  const columns: { column: number; boxes: string[] }[] = [];
  const stubById = new Map(stubs.map((stub) => [stub.id, stub]));
  for (const at of drawnColumns) {
    const ids = [...(order.get(at) ?? [])];
    const hidden = more.get(at);
    if (hidden !== undefined) {
      ids.push(`m:${String(at)}`);
    }
    columns.push({ column: at, boxes: ids });
    ids.forEach((id, index) => {
      const entry = walkedById.get(id);
      const stub = stubById.get(id);
      let content: BoxContent;
      if (entry !== undefined) {
        content = { type: "walked", walked: entry };
      } else if (stub !== undefined) {
        content = { type: "stub", stub };
      } else {
        content = { type: "more", hidden: hidden?.hidden ?? 0, hiddenWalked: hidden?.hiddenWalked ?? 0, column: at };
      }
      boxes.push({ id, content, column: at, row: index, x: COLUMN_STEP * at, y: ROW_STEP * index });
    });
  }

  const positionOf = positionsIn(typeOrder);
  const layoutEdges: LayoutEdge[] = [];
  edges.forEach((edge, index) => {
    const pair = ends[index];
    if (pair === undefined || !drawn.has(pair.src) || !drawn.has(pair.dst)) {
      return;
    }
    const fromDst = followsIn(view.types, edge.type);
    const source = fromDst ? pair.dst : pair.src;
    const target = fromDst ? pair.src : pair.dst;
    const next = (column.get(target) ?? 0) === (column.get(source) ?? 0) + 1;
    layoutEdges.push({
      id: `e:${String(index)}`,
      edge,
      source,
      target,
      sourceHandle: next ? "out" : "out-top",
      targetHandle: next ? "in" : "in-top",
      arrowAt: fromDst ? "start" : "end",
      position: positionOf(edge.type),
    });
  });

  let limit: CanvasLimit | null = null;
  if (cut) {
    const shown = boxes.reduce((count, box) => count + (box.content.type === "walked" ? box.content.walked.holders.length : 0), 0);
    limit = { lastColumn: drawnColumns[drawnColumns.length - 1] ?? 0, shown, total: view.nodes.length };
  }
  return { boxes, edges: layoutEdges, columns, limit };
}

/** The handles every box carries, at its side middles (8 x 8 px). */
export const BOX_HANDLES = [
  { id: "out", type: "source", side: "right", x: BOX_WIDTH - HANDLE_SIZE / 2, y: BOX_HEIGHT / 2 - HANDLE_SIZE / 2 },
  { id: "in", type: "target", side: "left", x: -HANDLE_SIZE / 2, y: BOX_HEIGHT / 2 - HANDLE_SIZE / 2 },
  { id: "out-top", type: "source", side: "top", x: BOX_WIDTH / 2 - HANDLE_SIZE / 2, y: -HANDLE_SIZE / 2 },
  { id: "in-top", type: "target", side: "top", x: BOX_WIDTH / 2 - HANDLE_SIZE / 2, y: -HANDLE_SIZE / 2 },
] as const;
