import type { FlowEdge, FlowGap, FlowNode, FlowView, TraceCounts } from "../api/types";
import { compareText, nodeName } from "../graph/layout";
import type { Translate } from "../i18n";
import type { Look } from "../ui/Badge";

// The Flow's closed tables and words (docs/features/flow-canvas.md "Data", "Gap rows";
// docs/features/trace-views.md). The one place under src/flow/ and src/trace/ that tells a gap state,
// a gap code, an edge's `kind` or a rollup apart: an edge's `kind` is the closed `FlowEdgeKind` of the
// daemon's schema, no project vocabulary, and a repeat is always dotted. A link `type` is never
// spelled: its colour and plain dash go by its place in the sorted types of the top read, which hold
// every edge, so folding moves no dash. A stage is project vocabulary, shown as served. Every word is
// the catalog's `flow` group; an edge's `kind`, `when` and `spec flow`'s line stay as served.

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

/** The gap codes the daemon writes, each with its catalog key. */
const GAP_MEANINGS = {
  "no-start": "noStart",
  "two-starts": "twoStarts",
  unreachable: "unreachable",
  "no-way-to-end": "noWayToEnd",
  "trap-cycle": "trapCycle",
  "unjoined-parallel": "unjoinedParallel",
  "cycle-no-repeat": "cycleNoRepeat",
  "definition-unused": "definitionUnused",
  "definition-once": "definitionOnce",
  "supply-unfed": "supplyUnfed",
} as const;

/** The ten gap codes, as the daemon writes them. */
export const GAP_CODES = Object.keys(GAP_MEANINGS) as readonly (keyof typeof GAP_MEANINGS)[];

/** What a gap row looks for; another code, nothing to add. */
export function gapMeaning(code: string, t: Translate): string | null {
  return Object.hasOwn(GAP_MEANINGS, code) ? t(`flow.gapMeaning.${GAP_MEANINGS[code as keyof typeof GAP_MEANINGS]}`) : null;
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
export function gapLook(gap: FlowGap, t: Translate): Look {
  if (gap.state === "measured" && gap.count !== null) {
    return gap.count === 0
      ? { label: t("flow.gap.none"), tone: "flow-gap-none", icon: "checkClean" }
      : { label: t("flow.gap.found", { count: gap.count }), tone: "flow-gap-found", icon: "flowGap" };
  }
  if (gap.state === "not_configured") {
    return { label: t("flow.gap.notConfigured"), tone: "flow-gap-not-configured", icon: "info" };
  }
  if (gap.state === "cannot") {
    return { label: t("flow.gap.cannotMeasure"), tone: "sync-cannot-verify", icon: "cannotCheck" };
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

function rowNamesIt(t: Translate): GapMark {
  return { shown: t("flow.gapMark.shown"), said: t("flow.gapMark.said") };
}

/**
 * A box's gap mark, the same on the canvas and in the List; null for none. A closed group counts what
 * it holds. An open group says only what names it: what it holds shows on its own boxes.
 */
export function gapMark(node: FlowNode, drawnOpen: boolean, named: ReadonlySet<string>, t: Translate): GapMark | null {
  const namesIt = named.has(nodeName(node));
  if (drawnOpen) {
    return namesIt ? rowNamesIt(t) : null;
  }
  if (node.holds > 0 && node.gaps > 0) {
    const text = withGapsText(node.gaps, t);
    return { shown: text, said: text };
  }
  return namesIt || node.gaps > 0 ? rowNamesIt(t) : null;
}

/** Every served row measured at 0: the rows have nothing to say. */
export function allClear(gaps: readonly FlowGap[]): boolean {
  return gaps.length > 0 && gaps.every((gap) => gap.state === "measured" && gap.count === 0);
}

/** `<n> rows need [flow] rules in specengine.toml`; null when none does. */
export function needsRulesLine(gaps: readonly FlowGap[], t: Translate): string | null {
  const rows = gaps.filter(needsRule).length;
  if (rows === 0) {
    return null;
  }
  return t("flow.needsRules", { count: rows });
}

/** `<k> of <n> boxes not drawn: close a group, or read the List`. */
export function notDrawnLine(notDrawn: number, total: number, t: Translate): string {
  return t("flow.notDrawn", { hidden: notDrawn, count: total });
}

/** `<n> document(s)` a closed group holds. */
export function holdsText(holds: number, t: Translate): string {
  return t("flow.holds", { count: holds });
}

/** `<g> with gaps`, the daemon's count for a group. */
export function withGapsText(gaps: number, t: Translate): string {
  return t("flow.withGaps", { count: gaps });
}

/** A start's word: at the top a flow starts, in a group's lane the flow enters. */
export function startWord(inLane: boolean, t: Translate): string {
  return inLane ? t("flow.entry") : t("flow.start");
}

/** `<n> box(es), <e> edge(s)` of a read. */
export function readCount(view: Pick<FlowView, "nodes" | "edges">, t: Translate): string {
  return `${t("flow.boxes", { count: view.nodes.length })}, ${t("flow.edges", { count: view.edges.length })}`;
}

/** A box's words as the panel, the canvas and the List say them. */
export function boxWords(node: FlowNode): string {
  return node.title ?? nodeName(node);
}

/** The word of a node with no card: a gap in the plan, drawn hollow wherever a step stands. */
const NO_CARD = "no card";

type RollupKey = "confirmed" | "inWork" | "suspect" | "departed" | "noCard";

/** The rollups as the daemon writes them, each with its catalog key, colour role and icon. */
const ROLLUPS: ReadonlyMap<string, Omit<Look, "label"> & { key: RollupKey }> = new Map([
  ["confirmed", { key: "confirmed", tone: "trace-confirmed", icon: "taskAccepted" }],
  ["in work", { key: "inWork", tone: "trace-in-work", icon: "taskInProgress" }],
  ["suspect", { key: "suspect", tone: "trace-suspect", icon: "specChanged" }],
  ["departed", { key: "departed", tone: "trace-departed", icon: "cardDeparted" }],
  [NO_CARD, { key: "noCard", tone: "trace-no-card", icon: "open" }],
] satisfies [string, Omit<Look, "label"> & { key: RollupKey }][]);

/** The five rollups, as the daemon writes them. */
export const ROLLUP_WORDS: readonly string[] = [...ROLLUPS.keys()];

/** How a rollup looks; one this build does not know raw and neutral (`docs/features/trace-views.md` "Flow badges"). */
export function rollupLook(rollup: string, t: Translate): Look {
  const known = ROLLUPS.get(rollup);
  return known === undefined ? { label: rollup, tone: "trace-unknown", icon: "unknown" } : { label: t(`flow.rollup.${known.key}`), tone: known.tone, icon: known.icon };
}

/** Whether a rollup is a step with no card: it stays in its place, hollow. */
export function isNoCard(rollup: string): boolean {
  return rollup === NO_CARD;
}

/** What each rollup says, for the legend: done, in work, in question, missing; `word` as the daemon writes it. */
export function rollupMeanings(t: Translate): readonly { word: string; look: Look; meaning: string }[] {
  return [...ROLLUPS].map(([word, { key }]) => ({ word, look: rollupLook(word, t), meaning: t(`flow.rollupMeaning.${key}`) }));
}

/** The parts of `counts`, by the rollup each state is, from what is done to what is missing. */
const COUNT_ORDER: readonly (readonly [keyof TraceCounts, string, RollupKey])[] = [
  ["confirmed", "confirmed", "confirmed"],
  ["in_work", "in work", "inWork"],
  ["suspect", "suspect", "suspect"],
  ["departed", "departed", "departed"],
  ["no_card", NO_CARD, "noCard"],
];

/** One state of `counts` with something in it. */
export interface CountPart {
  key: keyof TraceCounts;
  count: number;
  /** As a line says it: `3 confirmed`. */
  words: string;
  look: Look;
  hollow: boolean;
}

/** The served counts, zero parts left out: nothing here adds them up. */
export function countParts(counts: TraceCounts, t: Translate): CountPart[] {
  return COUNT_ORDER.flatMap(([key, word, words]) => {
    const count = counts[key];
    return count > 0 ? [{ key, count, words: t(`flow.count.${words}`, { count }), look: rollupLook(word, t), hollow: isNoCard(word) }] : [];
  });
}

/** `3 confirmed, 1 suspect, 2 no card`, as a closed group says it. */
export function countsText(counts: TraceCounts, t: Translate): string {
  return countParts(counts, t)
    .map((part) => part.words)
    .join(", ");
}

/** A lane's stage as a person reads it; none: the lane for what has no `stage:`. */
export function stageWords(stage: string | null, t: Translate): string {
  return stage ?? t("flow.noStage");
}

/** Where a step stands on the walk, counted from one; a distance the read did not give is not known, never a place. */
export function stepWords(distance: number | null, t: Translate): string {
  return distance === null ? t("flow.stepUnknown") : t("flow.step", { n: distance + 1 });
}

/** The lane's score: `<c> of <n> confirmed`. */
export function confirmedLine(confirmed: number, total: number, t: Translate): string {
  return t("flow.confirmed", { confirmed, count: total });
}

/** No step has a card: a card may still stand elsewhere, on a target gone, and a note then says so. */
export function noCardsYet(t: Translate): string {
  return t("flow.noCardsYet");
}

/** Beside a re-read that failed: what stands under it is the last read's, not the state now. */
export function lastReadWords(t: Translate): string {
  return t("flow.lastRead");
}

/** Beside the top read's failed re-read, on a tab drawing boxes: of those, only the top level's are that read's. */
export function statesHiddenWords(t: Translate): string {
  return t("flow.statesHidden");
}

/** A gap row whose read failed: it says nothing, least of all None. */
export function gapNotRead(t: Translate): Look {
  return { label: t("flow.gap.notRead"), tone: "flow-gap-unknown", icon: "unknown" };
}

/** No document has a stage: one lane, and what would give it others. */
export function noStagesYet(t: Translate): string {
  return t("flow.noStagesYet");
}
