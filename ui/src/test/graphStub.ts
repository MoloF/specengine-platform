import type { GraphOptions } from "../api/client";
import type { FollowedType, GraphEdge, GraphNode, GraphView } from "../api/types";
import { aGraphEdge, aGraphNode, aGraphView } from "./builders";
import { stubClient } from "./stubClient";

// A small graph on the stub client for the Graph view's tests: R-0 reaches three nodes at
// distance 1 and two at distance 2, with a back edge, an unresolved end, a skipped one and a
// nested section named by itself. Types are invented and in no sorted order; Impact follows its
// own list. R-404 names nothing (the exit-1 document); LONE reaches nothing; any other REF is
// answered with R-0's walk (a stub: it does not re-walk).

/** Markup an author pasted into a title; it must show as text. */
export const HOSTILE = "<img src=x onerror=\u0061lert(1)>";

/** A written end that must never become an href. */
export const HOSTILE_LINK = "javascript:\u0061lert(1)";

export const OUTGOING_TYPES: FollowedType[] = [
  { type: "zeta_type", direction: "out" },
  { type: "alpha_type", direction: "out" },
  { type: "gamma_type", direction: "out" },
];

export const IMPACT_TYPES: FollowedType[] = [
  { type: "gamma_type", direction: "in" },
  { type: "zeta_type", direction: "out" },
];

export const GRAPH_NODES: GraphNode[] = [
  aGraphNode({ id: "R-0", distance: 0, kind: "widget", title: "The root widget", path: "docs/spec/r-0.md" }),
  aGraphNode({ id: "A1", distance: 1, kind: "gadget", title: "First gadget", path: "docs/spec/a1.md" }),
  aGraphNode({ id: "A2", distance: 1, kind: "gadget", title: HOSTILE, path: "docs/spec/a2.md" }),
  aGraphNode({ id: "A3", distance: 1, kind: "gadget", title: "Third gadget", path: "docs/spec/a3.md", archived: true }),
  aGraphNode({ id: "B1", distance: 2, kind: "gizmo", title: "A gizmo", path: "docs/spec/b1.md" }),
  aGraphNode({ id: "B2", distance: 2, kind: null, title: null, path: "docs/spec/b2.md" }),
];

export const GRAPH_EDGES: GraphEdge[] = [
  aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A1", line: 3 }),
  aGraphEdge({ src: "R-0", type: "alpha_type", dst: "A2", line: 4 }),
  aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A3", line: 5 }),
  aGraphEdge({ src: "A1", type: "zeta_type", dst: "B1" }),
  aGraphEdge({ src: "A3", type: "gamma_type", dst: "B2" }),
  aGraphEdge({ src: "B1", type: "zeta_type", dst: "R-0", line: 6 }),
  aGraphEdge({ src: "A2", type: "zeta_type", dst: null, written: HOSTILE_LINK, state: "dangling", reason: "`R-404` resolves to no ID and no alias" }),
  aGraphEdge({ src: "B2", type: "alpha_type", dst: null, written: "other:R-9", state: "skipped" }),
  aGraphEdge({ src: "SEC-R", type: "gamma_type", dst: "A1", path: "docs/spec/r-0.md", line: 12 }),
];

/** What the stub's getGraph answers for a read. */
export function graphOf(options: GraphOptions): GraphView {
  const echo = {
    ref: options.ref,
    impact: options.impact ?? false,
    depth: options.depth ?? null,
    archive: options.archive ?? false,
  };
  if (options.ref === "R-404") {
    return aGraphView([], [], {
      ...echo,
      types: [],
      reason: "`R-404` resolves to no ID and no alias",
      notes: ["the index was refreshed before the walk"],
    });
  }
  if (options.ref === "LONE") {
    return aGraphView([aGraphNode({ id: "LONE", distance: 0 })], [], { ...echo, types: options.impact === true ? IMPACT_TYPES : OUTGOING_TYPES });
  }
  const table = options.impact === true ? IMPACT_TYPES : OUTGOING_TYPES;
  const types =
    options.types === undefined
      ? table
      : options.types.map((type) => table.find((entry) => entry.type === type) ?? { type, direction: options.impact === true ? "in" : "out" });
  const followed = new Set(types.map((entry) => entry.type));
  const edges = GRAPH_EDGES.filter((edge) => followed.has(edge.type));
  const names = new Set(edges.flatMap((edge) => [edge.src, edge.dst]));
  const nodes = GRAPH_NODES.filter((node) => node.distance === 0 || names.has(node.id));
  return aGraphView(nodes, edges, {
    ...echo,
    types,
    left_out: options.archive === true ? { generated: 1, tier3: 0 } : { generated: 1, tier3: 2 },
  });
}

/** A stub client serving the graph above. */
export function graphClient() {
  const client = stubClient();
  client.getGraph.mockImplementation((_project, options) => Promise.resolve(graphOf(options)));
  return client;
}
