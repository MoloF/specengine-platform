import type { TraceOptions } from "../api/client";
import type { FlowView, TraceCard, TraceCounts, TraceDocument, TraceNode, TraceStage, TraceUse } from "../api/types";
import type { MockCorpus } from "./build";
import { goneUnder, markOver, type MockBoard, type Settled, type StoredCard } from "./cards";
import { nameOf } from "./corpus";
import type { MockFlow } from "./flow";

// `spec trace` over the mock (docs/canon/spec-trace.md "The read", docs/features/trace-views.md
// "Data"): each read joins cards and spec, as the CLI does, and the mock is the one place that may.
// The scope, a node's state, its counts over what it holds, the lanes by stage; `spec flow --trace`
// joins them after the flow's read. Nothing here is stored.

/** harbor-sim's `stage:` keys: project vocabulary nothing resolves, cutting across its groups, as no `parent:` can; most documents have none, MEC-NIGHT-PASSAGE among the steps. */
export const HARBOR_STAGES: ReadonlyMap<string, string> = new Map([
  ["RULE-HIGH-WATER", "landfall"],
  ["RULE-SPRING-WINDOW", "landfall"],
  ["MEC-PILOTAGE", "landfall"],
  ["MEC-FAIRWAY", "passage"],
  ["MEC-MOORING", "quayside"],
  ["MEC-OLD-QUAYS", "quayside"],
]);

/** What a read takes besides the cards. */
export interface TraceWorld {
  flow: MockFlow;
  corpus: MockCorpus;
  /** Each document's steps from the starts. */
  distance: ReadonlyMap<string, number | null>;
  settled: Settled;
}

/** A node a read can list: a live document, or a section a card targets. */
interface Place {
  name: string;
  title: string | null;
  path: string;
  line: number;
  /** A document's `parent:`; a section has none. */
  parent: string | null;
  stage: string | null;
  distance: number | null;
  /** Its name and its sections': what `within` holds. */
  within: string[];
  document: boolean;
  /** A step of the flow, a section of one too. */
  step: boolean;
}

const NO_COUNTS: TraceCounts = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

/** A node's state from its cards' marks, first match. */
function rollupOf(marks: readonly TraceCard[]): keyof TraceCounts {
  if (marks.length === 0) {
    return "no_card";
  }
  if (marks.some(({ mark }) => mark === "departed")) {
    return "departed";
  }
  if (marks.some(({ mark }) => mark === "suspect")) {
    return "suspect";
  }
  return marks.some(({ mark }) => mark === "open" || mark === "correcting") ? "in_work" : "confirmed";
}

/** A state as a read words it. */
const WORDS: Record<keyof TraceCounts, string> = { no_card: "no card", in_work: "in work", suspect: "suspect", departed: "departed", confirmed: "confirmed" };

function counted(sum: TraceCounts, state: keyof TraceCounts): TraceCounts {
  return { ...sum, [state]: sum[state] + 1 };
}

/** Every document and section of the flow's project, as a read can list them. */
function placesOf({ flow, corpus, distance }: TraceWorld): Place[] {
  const sectionsOf = new Map(corpus.documents.map((file) => [nameOf(file.node), file.sections]));
  const steps = new Set([...flow.links.filter((link) => link.type === "follows").flatMap((link) => [link.from, link.to]), ...flow.steps, ...flow.starts, ...flow.ends]);
  return flow.documents.flatMap((document): Place[] => {
    const sections = sectionsOf.get(document.name) ?? [];
    const reached = distance.get(document.name) ?? null;
    const own = { path: document.path, stage: document.stage, distance: reached, parent: null, step: steps.has(document.name) };
    return [
      { ...own, name: document.name, title: document.title, line: document.line, parent: document.parent, within: [document.name, ...sections.map(nameOf)], document: true },
      ...sections.map((section): Place => ({ ...own, name: nameOf(section), title: section.title, line: section.line, within: [nameOf(section)], document: false })),
    ];
  });
}

/** `at` and the documents it holds by `parent:`, each once. */
function heldBy(at: Place, places: readonly Place[]): Place[] {
  const held = new Map([[at.name, at]]);
  for (const place of held.values()) {
    for (const child of places) {
      if (child.document && child.parent === place.name && !held.has(child.name)) {
        held.set(child.name, child);
      }
    }
  }
  return [...held.values()];
}

/** The lanes of the listed nodes, by (`distance`, unreached last, stage), the nodes with no stage last. */
function lanesOf(rows: readonly TraceNode[], states: ReadonlyMap<TraceNode, keyof TraceCounts>): TraceStage[] {
  const lanes = new Map<string | null, TraceStage>();
  for (const row of rows) {
    const lane = lanes.get(row.stage) ?? { stage: row.stage, distance: null, counts: NO_COUNTS };
    const state = states.get(row);
    lanes.set(row.stage, {
      ...lane,
      distance: row.distance === null ? lane.distance : Math.min(lane.distance ?? Infinity, row.distance),
      counts: state === undefined ? lane.counts : counted(lane.counts, state),
    });
  }
  const rank = ({ stage, distance }: TraceStage) => (stage === null ? Infinity : (distance ?? Number.MAX_SAFE_INTEGER));
  return [...lanes.values()].sort((a, b) => rank(a) - rank(b) || ((a.stage ?? "") < (b.stage ?? "") ? -1 : 1));
}

/** `spec trace [REF] [--stage S] [--summary] --json`; a REF naming nothing, its exit-1 document. */
export function traceOf(board: MockBoard, options: TraceOptions, world: TraceWorld): TraceDocument {
  const standing = board.cards.filter((card) => card.cancellation === null);
  const places = placesOf(world);
  const root = options.ref === undefined ? undefined : places.find((place) => place.name === options.ref);
  if (options.ref !== undefined && root === undefined) {
    return { ref: options.ref, reason: `no node \`${options.ref}\``, notes: [], counts: NO_COUNTS, stages: [], nodes: [], truncated: false, omitted: null };
  }

  const cardsOf = (place: Place): StoredCard[] => standing.filter((card) => card.targets.some((target) => place.within.includes(target)));
  const marksOf = (place: Place): TraceCard[] => cardsOf(place).map((card) => markOver(board, card, card.targets.filter((target) => place.within.includes(target)), world.settled));
  const stateOf = (place: Place): keyof TraceCounts => rollupOf(marksOf(place));
  const targeted = new Set(standing.flatMap((card) => card.targets));
  const listed = root === undefined ? places.filter((place) => place.document || targeted.has(place.name)) : heldBy(root, places);
  listed.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : a.line - b.line));

  const usesOf = (place: Place): TraceUse[] =>
    world.flow.links.flatMap((link) => {
      const user = link.type === "uses" && place.within.includes(link.to) ? places.find((other) => other.name === link.from) : undefined;
      return user === undefined ? [] : [{ id: user.name, rollup: WORDS[stateOf(user)], cards: cardsOf(user).length }];
    });

  const states = new Map<TraceNode, keyof TraceCounts>();
  // A lane holds steps and documents with a `stage:`: a document that is neither stands in none.
  const laned = new Set<TraceNode>();
  const rows = listed.map((place): TraceNode => {
    const state = stateOf(place);
    const held = place.document ? heldBy(place, places) : [place];
    const row: TraceNode = {
      id: place.name,
      title: place.title,
      path: place.path,
      stage: place.stage,
      distance: place.distance,
      rollup: WORDS[state],
      counts: held.reduce<TraceCounts>((sum, one) => counted(sum, stateOf(one)), NO_COUNTS),
      cards: marksOf(place),
      uses: usesOf(place),
    };
    states.set(row, state);
    if (place.step || place.stage !== null) {
      laned.add(row);
    }
    return row;
  });

  const kept = rows.filter((row) => options.summary !== true && (options.stage === undefined || (options.stage === "-" ? row.stage === null && laned.has(row) : row.stage === options.stage)));
  const listedCards = new Set(rows.flatMap((row) => row.cards.map((card) => card.id)));
  const notes = standing.filter((card) => root === undefined || listedCards.has(card.id)).flatMap((card) => goneUnder(board, card).map((target) => `${card.id}: \`${target}\` no longer resolves`));
  return {
    ref: options.ref ?? null,
    reason: null,
    notes,
    counts: rows.reduce<TraceCounts>((sum, row) => {
      const state = states.get(row);
      return state === undefined ? sum : counted(sum, state);
    }, NO_COUNTS),
    stages: lanesOf([...laned], states),
    nodes: kept,
    truncated: false,
    omitted: null,
  };
}

/** `spec flow --trace`: a read's boxes, each with its row's `rollup` and `counts`. */
export function withTrace(view: FlowView, trace: TraceDocument): FlowView {
  const rows = new Map(trace.nodes.map((row) => [row.id, row]));
  return {
    ...view,
    nodes: view.nodes.map((node) => {
      const row = rows.get(node.id ?? node.path);
      return row === undefined ? node : { ...node, rollup: row.rollup, counts: row.counts };
    }),
  };
}
