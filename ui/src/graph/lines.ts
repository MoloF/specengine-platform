import type { Direction, GraphEdge, GraphView } from "../api/types";
import type { IconName } from "../ui/Icon";
import { directionLabel } from "../tree/labels";
import type { Walked } from "./layout";

// The words the graph screen says about one answer (docs/features/ui-graph.md "Canvas", "List",
// "States"). Types and kinds are corpus vocabulary, shown as written; directions and link states
// are closed tables.

/**
 * An edge as one line: `<src, else -> --<type>--> <dst, else written> | <path>:<line>[ | <state>: <reason>]`,
 * the state part for an unresolved edge or one with a reason.
 */
export function edgeLine(edge: GraphEdge): string {
  const head = `${edge.src ?? "-"} --${edge.type}--> ${edge.dst ?? edge.written} | ${edge.path}:${String(edge.line)}`;
  if (edge.state === "resolved" && edge.reason === null) {
    return head;
  }
  return `${head} | ${edge.state}${edge.reason === null ? "" : `: ${edge.reason}`}`;
}

/** An edge's accessible name: `<src> <type> <dst, else written>`. */
export function edgeLabel(edge: GraphEdge): string {
  return `${edge.src ?? "-"} ${edge.type} ${edge.dst ?? edge.written}`;
}

/** Whether any holder of a walked name lies in an archived file. */
export function isArchived(walked: Walked): boolean {
  return walked.holders.some((holder) => holder.archived);
}

/** A walked box's accessible name: `<name>, <kind>, <title>, distance <d>[, focus][, archived]`. */
export function boxLabel(walked: Walked): string {
  const first = walked.holders[0];
  const parts = [walked.name, first?.kind ?? "no kind", first?.title ?? "no title", `distance ${String(walked.distance)}`];
  if (walked.distance === 0) {
    parts.push("focus");
  }
  if (isArchived(walked)) {
    parts.push("archived");
  }
  return parts.join(", ");
}

/** "+k more at distance d". */
export function moreLabel(hidden: number, column: number): string {
  return `+${String(hidden)} more at distance ${String(column)}`;
}

/** "<n> nodes, <e> edges", singular where one. */
export function countText(view: Pick<GraphView, "nodes" | "edges">): string {
  const nodes = view.nodes.length;
  const edges = view.edges.length;
  return `${String(nodes)} ${nodes === 1 ? "node" : "nodes"}, ${String(edges)} ${edges === 1 ? "edge" : "edges"}`;
}

type KnownDirection = "in" | "out";

const DIRECTION_ICON: Record<KnownDirection, IconName> = { out: "arrowRight", in: "arrowLeft" };

/** A direction's icon; an unknown direction takes the neutral one, its text shown raw. */
export function directionIcon(direction: Direction): IconName {
  return Object.hasOwn(DIRECTION_ICON, direction) ? DIRECTION_ICON[direction as KnownDirection] : "unknown";
}

/** "<type> (outgoing|incoming)", another direction raw. */
export function followedLabel(type: string, direction: Direction): string {
  return `${type} (${directionLabel(direction)})`;
}
