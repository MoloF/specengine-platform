import type { FlowEdge, FlowNode, FlowView } from "../api/types";
import type { FlowInput, FlowLayout } from "../flow/layout";
import { aFlowEdge, aFlowNode, aFlowView } from "./builders";
import { stubClient } from "./stubClient";

// Flows for the Flow's tests: reads as a daemon serves them, on invented names and types. The types
// are never a real link type's name, so a test passing here cannot lean on one.

/** Two types of no project's, sorted as `typesOf` gives them. */
export const TYPE_A = "alpha_link";
export const TYPE_B = "beta_link";

/** A stub client answering `getFlow` from reads: the top, and each group's by name; a group read not listed rejects nothing, it answers its name alone. */
export function flowClient(top: FlowView, groups: Record<string, FlowView> = {}) {
  const client = stubClient();
  client.getFlow.mockImplementation((_project, options) => {
    if (options.root === undefined) {
      return Promise.resolve(structuredClone(top));
    }
    const read = groups[options.root];
    return Promise.resolve(structuredClone(read ?? aFlowView([aFlowNode({ id: options.root, distance: null })], [], { root: options.root, depth: 1 })));
  });
  return client;
}

/** A group's read: the group itself, its children, the edges among them. */
export function aGroupRead(group: string, nodes: FlowNode[], edges: FlowEdge[] = [], fields: Partial<FlowView> = {}): FlowView {
  return aFlowView([aFlowNode({ id: group, distance: null }), ...nodes.map((node) => ({ ...node, group }))], edges, { root: group, depth: 1, ...fields });
}

/** The same small step-by-step flow read: a line of boxes with `distance` 0, 1, 2. */
export function aLine(ids: readonly string[]): FlowNode[] {
  return ids.map((id, index) => aFlowNode({ id, distance: index }));
}

/** A deterministic sequence of small numbers, so the trial's flow is the same on every run. */
function sequence(seed: number): (limit: number) => number {
  let state = seed;
  return (limit) => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state % limit;
  };
}

/**
 * The trial of owner's decision 11 (AC-12 of docs/features/flow-canvas.md): 200 boxes, 12 columns,
 * 30 off the walk, 40 edges against it. Forward edges reach the next column, some skip one or two,
 * and every 5th box off the walk feeds a step while the others are used by two.
 */
export function flow200(): FlowInput {
  const next = sequence(20_260_101);
  const widths = [2, 6, 10, 14, 18, 22, 24, 22, 18, 14, 12, 8];
  const columns: string[][] = [];
  const nodes: FlowNode[] = [];
  widths.forEach((width, distance) => {
    const column: string[] = [];
    for (let row = 0; row < width; row += 1) {
      const id = `S-${String(distance).padStart(2, "0")}-${String(row).padStart(2, "0")}`;
      column.push(id);
      nodes.push(aFlowNode({ id, distance, start: distance === 0 }));
    }
    columns.push(column);
  });
  const pick = (column: number): string => {
    const ids = columns[column] ?? [];
    return ids[next(ids.length)] ?? "";
  };
  const edges: FlowEdge[] = [];
  columns.forEach((ids, column) => {
    if (column === 0) {
      return;
    }
    for (const id of ids) {
      edges.push(aFlowEdge({ from: pick(column - 1), to: id }));
      if (next(4) === 0 && column >= 3) {
        edges.push(aFlowEdge({ from: pick(column - 2 - next(2)), to: id, kind: "alternative", when: "the long way round" }));
      }
    }
  });
  const backward = new Set<string>();
  while (backward.size < 40) {
    const from = 2 + next(10);
    const to = next(from);
    const edge = aFlowEdge({ from: pick(from), to: pick(to), kind: backward.size % 2 === 0 ? "repeat" : null });
    const key = `${edge.from}>${edge.to}`;
    if (!backward.has(key)) {
      backward.add(key);
      edges.push(edge);
    }
  }
  for (let index = 0; index < 30; index += 1) {
    const id = `D-${String(index).padStart(2, "0")}`;
    nodes.push(aFlowNode({ id, distance: null }));
    if (index % 5 === 0) {
      edges.push(aFlowEdge({ from: id, to: pick(next(12)), type: TYPE_B }));
    } else {
      edges.push(aFlowEdge({ from: pick(next(12)), to: id, type: TYPE_B }), aFlowEdge({ from: pick(next(12)), to: id, type: TYPE_B }));
    }
  }
  const seen = new Set<string>();
  const unique = edges.filter((edge) => {
    const key = `${edge.from}>${edge.to}:${edge.type}:${edge.kind ?? ""}`;
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
  return { top: aFlowView(nodes, unique), groups: new Map(), open: new Set() };
}

/** The crossings of forward edges inside the gaps: two curves over one gap whose order swaps. */
export function forwardCrossings(layout: FlowLayout): number {
  const inGap = new Map<string, { from: number; to: number }[]>();
  for (const laid of layout.edges) {
    if (laid.route !== "forward") {
      continue;
    }
    for (let index = 1; index + 1 < laid.points.length; index += 2) {
      const left = laid.points[index];
      const right = laid.points[index + 1];
      if (left !== undefined && right !== undefined) {
        const key = `${laid.group ?? ""}:${String(left.x)}`;
        inGap.set(key, [...(inGap.get(key) ?? []), { from: left.y, to: right.y }]);
      }
    }
  }
  let crossings = 0;
  for (const spans of inGap.values()) {
    spans.forEach((span, index) => {
      for (const other of spans.slice(index + 1)) {
        if ((span.from - other.from) * (span.to - other.to) < 0) {
          crossings += 1;
        }
      }
    });
  }
  return crossings;
}
