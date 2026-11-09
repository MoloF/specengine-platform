import type { Direction, GraphEdge, GraphView } from "../api/types";
import type { IconName } from "../ui/Icon";
import type { Translate } from "../i18n";
import { directionLabel } from "../tree/labels";
import type { Walked } from "./layout";

// The words the graph screen says about one answer (ui/README.md "Screen rules", Canvas and
// States; docs/features/ui-graph.md "Implementation", List). Types and kinds are corpus
// vocabulary, shown as written; directions and link states are closed tables.

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
export function boxLabel(walked: Walked, t: Translate): string {
  const first = walked.holders[0];
  const parts = [walked.name, first?.kind ?? t("graph.noKind"), first?.title ?? t("graph.noTitle"), t("graph.distance", { distance: walked.distance })];
  if (walked.distance === 0) {
    parts.push(t("graph.focusWord"));
  }
  if (isArchived(walked)) {
    parts.push(t("graph.archivedWord"));
  }
  return parts.join(", ");
}

/** "+k more at distance d". */
export function moreLabel(hidden: number, column: number, t: Translate): string {
  return t("graph.more", { hidden, column });
}

/** "<n> nodes, <e> edges": each count its own plural group. */
export function countText(view: Pick<GraphView, "nodes" | "edges">, t: Translate): string {
  return t("graph.counts", { nodes: t("graph.nodes", { count: view.nodes.length }), edges: t("graph.edges", { count: view.edges.length }) });
}

type KnownDirection = "in" | "out";

const DIRECTION_ICON: Record<KnownDirection, IconName> = { out: "arrowRight", in: "arrowLeft" };

/** A direction's icon; an unknown direction takes the neutral one, its text shown raw. */
export function directionIcon(direction: Direction): IconName {
  return Object.hasOwn(DIRECTION_ICON, direction) ? DIRECTION_ICON[direction as KnownDirection] : "unknown";
}

/** "<type> (outgoing|incoming)", another direction raw. */
export function followedLabel(type: string, direction: Direction, t: Translate): string {
  return `${type} (${directionLabel(direction, t)})`;
}
