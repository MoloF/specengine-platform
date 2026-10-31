import type { FlowEdge, FlowGap, FlowNode, FlowView } from "../api/types";
import { compareText, nodeName } from "../graph/layout";
import type { Look } from "../ui/Badge";

// The Flow's closed tables and words (docs/features/flow-canvas.md "Data", "Gap rows"). The one
// place under src/flow/ that tells a gap state, a gap code or an edge's `kind` apart: an edge's
// `kind` is the closed `FlowEdgeKind` of the daemon's schema, no project vocabulary, and a repeat is
// always dotted. A link `type` is never spelled: its colour and plain dash go by its place in the
// sorted types of the top read, which hold every edge, so folding moves no dash.

/** The seven stroke patterns: four by a type's place, then the three kinds; `repeat` alone is dotted. */
export const FLOW_DASHES = ["none", "14 6", "24 5 6 5", "6 5 6 14", "9 5", "18 4 3 4", "1 7"] as const;

const TYPE_DASHES = FLOW_DASHES.slice(0, 4);

const KIND_DASHES: Record<string, string> = {
  alternative: FLOW_DASHES[4],
  parallel: FLOW_DASHES[5],
  repeat: FLOW_DASHES[6],
};

/** The kinds the legend always names, as the daemon writes them. */
export const FLOW_KINDS: readonly string[] = Object.keys(KIND_DASHES);

/** The edge colours of tokens.css, `--flow-edge-0` to `-3`. */
const EDGE_COLOURS = 4;

/** A type's place, wrapped; a type the order does not hold takes the first. */
function placeOf(type: string, types: readonly string[], size: number): number {
  return Math.max(0, types.indexOf(type)) % size;
}

/** Every type a project's flow holds, sorted: the top read's edges and each node's `inner`. */
export function typesOf(top: FlowView): string[] {
  const types = new Set(top.edges.map((edge) => edge.type));
  for (const node of top.nodes) {
    for (const inner of node.inner) {
      types.add(inner.type);
    }
  }
  return [...types].sort(compareText);
}

/** A type's plain stroke pattern, by its place. */
export function typeDash(type: string, types: readonly string[]): string {
  return TYPE_DASHES[placeOf(type, types, TYPE_DASHES.length)] ?? "none";
}

/** An edge's stroke pattern: its kind's, whatever the types; an unknown kind or none its type's. */
export function dashOf(edge: FlowEdge, types: readonly string[]): string {
  const dash = edge.kind === null || !Object.hasOwn(KIND_DASHES, edge.kind) ? undefined : KIND_DASHES[edge.kind];
  return dash ?? typeDash(edge.type, types);
}

/** A repeat, drawn heavier: its dots have to read at the smallest zoom. */
export function isRepeat(edge: FlowEdge): boolean {
  return edge.kind === "repeat";
}

/** A type's colour role. */
export function typeColour(type: string, types: readonly string[]): string {
  return `var(--flow-edge-${String(placeOf(type, types, EDGE_COLOURS))})`;
}

/** The longest `when` an edge shows on the canvas; the List has it whole. */
export const WHEN_SHOWN = 24;

/** `when` for the canvas: its first characters, whole ones, then an ellipsis. */
export function shortWhen(when: string): string {
  const characters = Array.from(when);
  return characters.length > WHEN_SHOWN ? `${characters.slice(0, WHEN_SHOWN).join("")}…` : when;
}

/** What an edge says on the canvas: `kind`, `when` clipped, `x<count>` above 1; empty for a plain edge. */
export function edgeWords(edge: FlowEdge): string[] {
  const words: string[] = [];
  if (edge.kind !== null) {
    words.push(edge.kind);
  }
  if (edge.when !== null) {
    words.push(shortWhen(edge.when));
  }
  if (edge.count > 1) {
    words.push(`x${String(edge.count)}`);
  }
  return words;
}

/** An edge as `spec flow` prints it: `<from> -> <to> | <type> | x<count>[ | kind <kind>][ | when <when>] | <path>:<line>`. */
export function edgeLine(edge: FlowEdge): string {
  const kind = edge.kind === null ? "" : ` | kind ${edge.kind}`;
  const when = edge.when === null ? "" : ` | when ${edge.when}`;
  return `${edge.from} -> ${edge.to} | ${edge.type} | x${String(edge.count)}${kind}${when} | ${edge.path}:${String(edge.line)}`;
}

/** An edge's accessible name: `<from> <type> <to>`, then what it says. */
export function edgeName(edge: FlowEdge): string {
  return [`${edge.from} ${edge.type} ${edge.to}`, ...edgeWords(edge)].join(", ");
}

const GAP_MEANINGS: Record<string, string> = {
  "no-start": "Nothing starts the flow.",
  "two-starts": "The flow starts in more than one place.",
  unreachable: "Steps no path from a start reaches.",
  "no-way-to-end": "Steps with no path to an end.",
  "trap-cycle": "Loops with no way out to an end.",
  "unjoined-parallel": "Parallel branches that never join.",
  "cycle-no-repeat": "Loops not marked as a repeat.",
  "definition-unused": "Definitions nothing uses.",
  "definition-once": "Definitions used only once.",
  "supply-unfed": "Supply loops nothing outside feeds.",
};

/** What a gap row looks for; another code, nothing to add. */
export function gapMeaning(code: string): string | null {
  return Object.hasOwn(GAP_MEANINGS, code) ? (GAP_MEANINGS[code] ?? null) : null;
}

/** `no-start`'s item is the read's scope, no document: it stays text, never a button. */
export function namesDocuments(gap: FlowGap): boolean {
  return gap.code !== "no-start";
}

const STATES: readonly string[] = ["measured", "not_configured", "cannot"];

/** A state the table holds; any other is shown raw, neutral, last. */
function isKnownState(gap: FlowGap): boolean {
  return STATES.includes(gap.state);
}

/** The rows as served, unknown states last. */
export function gapsInOrder(gaps: readonly FlowGap[]): FlowGap[] {
  return [...gaps.filter(isKnownState), ...gaps.filter((gap) => !isKnownState(gap))];
}

/** Whether a row waits for a `[flow]` rule. */
export function needsRule(gap: FlowGap): boolean {
  return gap.state === "not_configured";
}

/** A row's state in words: never a number it was not given. */
export function gapLook(gap: FlowGap): Look {
  if (gap.state === "measured" && gap.count !== null) {
    return gap.count === 0
      ? { label: "None", tone: "flow-gap-none", icon: "checkClean" }
      : { label: `${String(gap.count)} found`, tone: "flow-gap-found", icon: "flowGap" };
  }
  if (gap.state === "not_configured") {
    return { label: "Not configured", tone: "flow-gap-not-configured", icon: "info" };
  }
  if (gap.state === "cannot") {
    return { label: "Cannot measure", tone: "sync-cannot-verify", icon: "cannotCheck" };
  }
  return { label: gap.state, tone: "flow-gap-unknown", icon: "unknown" };
}

/** Every document a row's items name, `no-start`'s scope left out. */
export function namedByRows(gaps: readonly FlowGap[]): Set<string> {
  return new Set(gaps.filter(namesDocuments).flatMap((gap) => gap.items.flat()));
}

/** How a box shows its gaps: the word on the box, and what it says aloud. */
export interface GapMark {
  shown: string;
  said: string;
}

const ROW_NAMES_IT: GapMark = { shown: "Gap", said: "named by a gap row" };

/**
 * A box's gap mark, the same on the canvas and in the List; null for none. A closed group counts what
 * it holds. An open group says only what names it: what it holds shows on its own boxes.
 */
export function gapMark(node: FlowNode, drawnOpen: boolean, named: ReadonlySet<string>): GapMark | null {
  const namesIt = named.has(nodeName(node));
  if (drawnOpen) {
    return namesIt ? ROW_NAMES_IT : null;
  }
  if (node.holds > 0 && node.gaps > 0) {
    const text = withGapsText(node.gaps);
    return { shown: text, said: text };
  }
  return namesIt || node.gaps > 0 ? ROW_NAMES_IT : null;
}

/** `<n> rows need [flow] rules in specengine.toml`; null when none does. */
export function needsRulesLine(gaps: readonly FlowGap[]): string | null {
  const rows = gaps.filter(needsRule).length;
  if (rows === 0) {
    return null;
  }
  return `${String(rows)} ${rows === 1 ? "row needs" : "rows need"} [flow] rules in specengine.toml`;
}

/** `<k> of <n> boxes not drawn: close a group, or read the List`. */
export function notDrawnLine(notDrawn: number, total: number): string {
  return `${String(notDrawn)} of ${String(total)} boxes not drawn: close a group, or read the List`;
}

/** `<n> document(s)` a closed group holds. */
export function holdsText(holds: number): string {
  return `${String(holds)} ${holds === 1 ? "document" : "documents"}`;
}

/** `<g> with gaps`, the daemon's count for a group. */
export function withGapsText(gaps: number): string {
  return `${String(gaps)} with gaps`;
}

/** A start's word: at the top a flow starts, in a group's lane the flow enters. */
export function startWord(inLane: boolean): string {
  return inLane ? "Entry" : "Start";
}

/** `<n> box(es), <e> edge(s)` of a read. */
export function readCount(view: Pick<FlowView, "nodes" | "edges">): string {
  const boxes = view.nodes.length;
  const edges = view.edges.length;
  return `${String(boxes)} ${boxes === 1 ? "box" : "boxes"}, ${String(edges)} ${edges === 1 ? "edge" : "edges"}`;
}

/** A box's words as the panel, the canvas and the List say them. */
export function boxWords(node: FlowNode): string {
  return node.title ?? nodeName(node);
}
