import { ClientError, type FlowOptions } from "../api/client";
import type { FlowEdge, FlowGap, FlowInner, FlowNode, FlowView } from "../api/types";
import type { MockCorpus } from "./build";
import { nameOf, resolveRef } from "./corpus";
import type { Scenario } from "./scenario";
import { HARBOR_STAGES } from "./trace";

// The flows of the mock projects (docs/features/flow-canvas.md "Data", Mock): `spec flow --json` as
// the daemon's browser view sends it (`docs/canon/spec-flow.md` "The read"), uncut. The scope, the
// walk, the fold and the lifted edges are worked out here, as the core does; the ten gap rows are
// written down, then kept to a read's scope. A mock is the one place that may lift an edge: the
// screen never does.
//
// harbor-sim, over its own live documents: DOM-HARBOR over DOM-WATER, MEC-PILOTAGE, MEC-FAIRWAY,
// DOM-BERTHS and DOM-PILOTS; the three link types and every `kind`; arcs against the walk with a
// kind and without one; a step that repeats itself; MEC-TIDE-TABLES, a definition two levels under
// DOM-WATER, the gap item `getTree` has to find; the berth limits, a child of DOM-BERTHS with no flow
// link, so only a rooted read holds it; the pilot station's steps in landfall, passage and quayside,
// so each of those stages spans two groups (docs/features/writer-studio.md "Mocks (`ui/src/mocks/`)").
// ledger-api: no `[flow]`, so its rows are `not_configured`. `empty`: no flow, the rows stay.
// `large`: 500 documents in 25 groups, generated, past the canvas's cap once a few open.
// `no-gaps`: harbor-sim's own links, its ten rows written down at None.
// `spec flow --trace` is joined after this read, in src/mocks/trace.ts, as the CLI joins it.

/** A live document as a read sees it; `stage` its front-matter's, project vocabulary. */
export interface FlowDocument {
  name: string;
  id: string | null;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  parent: string | null;
  stage: string | null;
}

/** A flow link: `from` and `to` in reading direction, as a read serves it. */
interface FlowLink {
  from: string;
  to: string;
  type: string;
  kind?: string;
  when?: string;
}

/** What a project's flow holds besides its documents. */
export interface MockFlow {
  documents: FlowDocument[];
  links: FlowLink[];
  /** Marked starts and ends (a `[flow]` table's picks). */
  starts: string[];
  ends: string[];
  /** Steps a `[[flow.step]]` table picks beyond those a `follows` link makes. */
  steps: string[];
  rows: FlowGap[];
  leftOut: { generated: number; tier3: number };
}

/** What `spec flow --stage` takes for the steps with no `stage:`. */
const NO_STAGE = "-";

/** A row's line above 0, as `spec flow` words it (`crates/specengine-cli/src/flow.rs` `remedy`). */
export const GAP_REMEDIES: Readonly<Record<string, string>> = {
  "no-start": "no step here is a start: match the flow's first step in [[flow.start]], or, reading a part, add a follows link into it from outside",
  "two-starts": "more than one start: narrow [[flow.start]] to one",
  unreachable: "no path from a start reaches it: add a follows link on it, naming a step before it",
  "no-way-to-end": "no path from it reaches an end: have a later step follow it, or match it in [[flow.end]]",
  "trap-cycle": "a loop with no way out: have a step outside it follow a member, or match a member in [[flow.end]]",
  "unjoined-parallel": "a parallel fork with one branch, or branches that never meet: add a branch, or a later step following them all",
  "cycle-no-repeat": "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}",
  "definition-unused": "no document uses it: add a uses link to it, or drop it from [[flow.definition]]",
  "definition-once": "only one document uses it: add a uses link from a second document, or fold it into that document",
  "supply-unfed": "these only supply each other: add a supplies link into them from a node outside",
};

/** A `not_configured` row's line by its table, as `spec flow` words it (`cause_text`). */
const NO_RULE = {
  start: "no [[flow.start]] rule: add one naming the first step by paths",
  end: "no [[flow.end]] rule: add one naming the last step by paths",
  definition: "no [[flow.definition]] rule: add one naming what steps share, by paths or kinds",
} as const;

/** A counted row: above 0 it carries its remedy, at 0 `null`, as the daemon sends it. */
const measured = (code: string, items: string[][] = []): FlowGap => ({ code, state: "measured", count: items.length, items, cause: items.length > 0 ? (GAP_REMEDIES[code] ?? null) : null });

const notConfigured = (code: string, table: keyof typeof NO_RULE): FlowGap => ({ code, state: "not_configured", count: null, items: [], cause: NO_RULE[table] });

const follows = (from: string, to: string, kind?: string, when?: string): FlowLink => ({ from, to, type: "follows", ...(kind === undefined ? {} : { kind }), ...(when === undefined ? {} : { when }) });

const HARBOR_LINKS: FlowLink[] = [
  follows("RULE-HIGH-WATER", "RULE-SPRING-WINDOW"),
  follows("RULE-SPRING-WINDOW", "MEC-PILOTAGE"),
  follows("MEC-PILOTAGE", "MEC-FAIRWAY"),
  follows("MEC-PILOTAGE", "MEC-NIGHT-PASSAGE", "alternative", "after dark"),
  follows("MEC-FAIRWAY", "MEC-MOORING", "alternative", "a berth is free at the quay"),
  follows("MEC-NIGHT-PASSAGE", "MEC-MOORING", "parallel"),
  // Against the walk: declared, and not.
  follows("MEC-MOORING", "MEC-PILOTAGE", "repeat", "the next ship calls"),
  follows("MEC-FAIRWAY", "MEC-PILOTAGE"),
  follows("RULE-SPRING-WINDOW", "RULE-SPRING-WINDOW", "repeat", "the next spring tide"),
  { from: "MEC-MOORING", to: "RULE-HIGH-WATER", type: "uses" },
  { from: "MEC-TIDE-TABLES", to: "RULE-SPRING-WINDOW", type: "supplies" },
  { from: "RULE-HIGH-WATER", to: "MEC-MOORING", type: "supplies" },
  // The pilot station: a call after the spring window, the log after the call, the return after mooring.
  follows("RULE-SPRING-WINDOW", "MEC-PILOT-CALL"),
  follows("MEC-PILOT-CALL", "MEC-HARBOUR-LOG"),
  follows("MEC-MOORING", "MEC-PILOT-RETURN"),
];

const HARBOR_ROWS: FlowGap[] = [
  measured("no-start"),
  measured("two-starts"),
  measured("unreachable"),
  measured("no-way-to-end"),
  measured("trap-cycle"),
  measured("unjoined-parallel", [["MEC-NIGHT-PASSAGE"]]),
  measured("cycle-no-repeat", [["MEC-FAIRWAY", "MEC-PILOTAGE"]]),
  measured("definition-unused", [["MEC-TIDE-TABLES"]]),
  measured("definition-once", [["RULE-HIGH-WATER"]]),
  measured("supply-unfed"),
];

/** `no-gaps`: every row measured at 0, the Flow's one line in place of the rows (docs/features/screen-fit.md). */
const NO_GAPS: FlowGap[] = HARBOR_ROWS.map((row) => measured(row.code));

const NO_RULES: FlowGap[] = [
  notConfigured("no-start", "start"),
  notConfigured("two-starts", "start"),
  notConfigured("unreachable", "start"),
  notConfigured("no-way-to-end", "end"),
  notConfigured("trap-cycle", "end"),
  measured("unjoined-parallel"),
  measured("cycle-no-repeat"),
  notConfigured("definition-unused", "definition"),
  notConfigured("definition-once", "definition"),
  measured("supply-unfed"),
];

/** A scope with no step: `no-start` names it, and `unreachable` cannot be measured under it. */
const NO_STEP: FlowGap[] = [
  measured("no-start", [["all"]]),
  measured("two-starts"),
  { code: "unreachable", state: "cannot", count: null, items: [], cause: "no start and no entering edge to walk from" },
  measured("no-way-to-end"),
  measured("trap-cycle"),
  measured("unjoined-parallel"),
  measured("cycle-no-repeat"),
  measured("definition-unused"),
  measured("definition-once"),
  measured("supply-unfed"),
];

const LEDGER_LINKS: FlowLink[] = [
  follows("POL-WRITES", "POL-REFUND-RULES"),
  follows("POL-REFUND-RULES", "EP-REFUNDS"),
  follows("EP-REFUNDS", "POL-REFUND-RULES", "repeat", "the amount is corrected"),
];

/** The live documents of a corpus: tier 3 and generated stay out, counted; `stages` the project's `stage:` keys. */
function documentsOf(corpus: MockCorpus, stages: ReadonlyMap<string, string>): { documents: FlowDocument[]; leftOut: { generated: number; tier3: number } } {
  const leftOut = { generated: 0, tier3: 0 };
  const documents: FlowDocument[] = [];
  for (const { node } of corpus.documents) {
    if (node.generated) {
      leftOut.generated += 1;
    } else if (node.archived) {
      leftOut.tier3 += 1;
    } else {
      documents.push({ name: nameOf(node), id: node.id, kind: node.kind, title: node.title, path: node.path, line: node.line, parent: node.parent, stage: stages.get(nameOf(node)) ?? null });
    }
  }
  return { documents, leftOut };
}

const pad = (value: number) => String(value).padStart(2, "0");

const STAGE_WORDS = ["Arrival", "Survey", "Unloading", "Sorting", "Storage", "Dispatch", "Repairs", "Refit"] as const;

/** `large`: 25 groups in a line, 19 steps each, a branch and a join in each, a loop back every sixth. */
function largeFlow(): MockFlow {
  const documents: FlowDocument[] = [];
  const links: FlowLink[] = [];
  const stray: string[][] = [];
  for (let group = 1; group <= 25; group += 1) {
    const name = `ZONE-GEN-${pad(group)}`;
    const word = STAGE_WORDS[group % STAGE_WORDS.length] ?? "Stage";
    documents.push({ name, id: name, kind: "domain", title: `${word} zone ${String(group)}`, path: `docs/spec/zones/z${pad(group)}/README.md`, line: 1, parent: null, stage: null });
    for (let step = 1; step <= 19; step += 1) {
      const id = `${name}-${pad(step)}`;
      documents.push({ name: id, id, kind: "mechanic", title: `${word} step ${String(step)}`, path: `docs/spec/zones/z${pad(group)}/s${pad(step)}.md`, line: 1, parent: name, stage: word.toLowerCase() });
      if (step > 1 && step !== 7 && step !== 8 && step !== 9 && step !== 15) {
        links.push(follows(`${name}-${pad(step - 1)}`, id));
      }
    }
    links.push(follows(`${name}-06`, `${name}-07`, "alternative", "the hold is dry"), follows(`${name}-06`, `${name}-08`, "alternative", "the hold is wet"));
    links.push(follows(`${name}-07`, `${name}-09`), follows(`${name}-08`, `${name}-09`), follows(`${name}-14`, `${name}-12`, "repeat", "another tally"), follows(`${name}-14`, `${name}-16`));
    if (group > 1) {
      links.push(follows(`ZONE-GEN-${pad(group - 1)}-19`, `${name}-01`));
    }
    if (group % 6 === 0) {
      links.push(follows(`${name}-19`, `ZONE-GEN-${pad(group - 3)}-01`, "repeat", "the next convoy"));
    }
    stray.push([`${name}-15`]);
  }
  const rows = HARBOR_ROWS.map((row) => (row.code === "unreachable" ? measured("unreachable", stray) : measured(row.code)));
  return { documents, links, starts: ["ZONE-GEN-01-01"], ends: ["ZONE-GEN-25-19"], steps: stray.flat(), rows, leftOut: { generated: 0, tier3: 0 } };
}

function harborFlow(corpus: MockCorpus, scenario: Scenario): MockFlow {
  if (scenario === "large") {
    return largeFlow();
  }
  const { documents, leftOut } = documentsOf(corpus, HARBOR_STAGES);
  const rows = documents.length === 0 ? NO_STEP : scenario === "no-gaps" ? NO_GAPS : HARBOR_ROWS;
  return { documents, links: HARBOR_LINKS, starts: ["RULE-HIGH-WATER"], ends: ["MEC-MOORING"], steps: [], rows, leftOut };
}

function ledgerFlow(corpus: MockCorpus, scenario: Scenario): MockFlow {
  const { documents, leftOut } = documentsOf(corpus, new Map());
  return { documents, links: scenario === "empty" ? [] : LEDGER_LINKS, starts: [], ends: [], steps: [], rows: NO_RULES, leftOut };
}

function byText(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * `spec flow` over a mock flow: the scope, the walk, the fold, the lifted edges, the rows kept to the
 * scope; then `stage` cuts the shown nodes after the fold and moves nothing else
 * (`docs/canon/spec-flow.md` "The read"): a node whose own stage is S, `-` a shown step with none,
 * never a group or a definition, then the edges whose two ends are kept.
 */
function readFlow(flow: MockFlow, options: FlowOptions): Omit<FlowView, "reason"> {
  const byName = new Map(flow.documents.map((document) => [document.name, document]));
  const kids = new Map<string, FlowDocument[]>();
  for (const document of flow.documents) {
    if (document.parent !== null && byName.has(document.parent)) {
      kids.set(document.parent, [...(kids.get(document.parent) ?? []), document]);
    }
  }
  const scope = new Set<string>();
  if (options.root !== undefined) {
    const stack = [options.root];
    for (let name = stack.pop(); name !== undefined; name = stack.pop()) {
      if (!scope.has(name)) {
        scope.add(name);
        stack.push(...(kids.get(name) ?? []).map((kid) => kid.name));
      }
    }
  } else {
    for (const link of flow.links) {
      scope.add(link.from);
      scope.add(link.to);
    }
    for (const name of [...flow.starts, ...flow.ends, ...flow.steps]) {
      scope.add(name);
    }
    for (const name of [...scope]) {
      for (let up = byName.get(name)?.parent ?? null; up !== null && byName.has(up); up = byName.get(up)?.parent ?? null) {
        scope.add(up);
      }
    }
  }
  const inScope = (name: string) => scope.has(name) && byName.has(name);
  const links = flow.links.filter((link) => inScope(link.from) && inScope(link.to));
  const forward = links.filter((link) => link.type === "follows");
  const next = new Map<string, string[]>();
  for (const link of forward) {
    next.set(link.from, [...(next.get(link.from) ?? []), link.to]);
  }
  const marked = flow.starts.filter(inScope);
  // A `follows` link crossing the scope still makes a step of its end inside: an entry or a way out.
  const crossing = flow.links.filter((link) => link.type === "follows").flatMap((link) => [link.from, link.to]);
  const steps = new Set([...crossing, ...flow.steps, ...marked, ...flow.ends].filter(inScope));
  const entered = new Set(forward.filter((link) => link.from !== link.to).map((link) => link.to));
  const starts = marked.length > 0 ? marked : [...steps].filter((name) => !entered.has(name));
  const distance = new Map(starts.map((name) => [name, 0]));
  const queue = [...starts];
  for (let at = queue.shift(); at !== undefined; at = queue.shift()) {
    for (const to of next.get(at) ?? []) {
      if (!distance.has(to)) {
        distance.set(to, (distance.get(at) ?? 0) + 1);
        queue.push(to);
      }
    }
  }

  // Levels by `parent:`, from the scope's roots; a document deeper than `depth` shows as its ancestor there.
  const rootOf = (name: string) => {
    const parent = byName.get(name)?.parent ?? null;
    return parent === null || !inScope(parent);
  };
  const level = new Map<string, number>();
  const levelOf = (name: string): number => {
    const known = level.get(name);
    if (known !== undefined) {
      return known;
    }
    const parent = byName.get(name)?.parent;
    const found = rootOf(name) || parent === undefined || parent === null || name === options.root ? 0 : levelOf(parent) + 1;
    level.set(name, found);
    return found;
  };
  const shownAs = (name: string): string => {
    let at = name;
    while (options.depth !== undefined && levelOf(at) > options.depth) {
      at = byName.get(at)?.parent ?? at;
    }
    return at;
  };

  // Under a ROOT the mock serves Top's rows, keeping the items with a member in scope: the mock's reading.
  const rows = flow.rows.map((row): FlowGap => {
    if (options.root === undefined || row.state !== "measured") {
      return row;
    }
    return measured(row.code, row.items.filter((item) => item.some(inScope)));
  });
  const named = new Set(rows.flatMap((row) => (row.code === "no-start" ? [] : row.items.flat())));
  const holds = new Map<string, number>();
  const least = new Map<string, number>();
  const gaps = new Map<string, number>();
  // A shown node is a start or an end when it or anything it holds is one (`docs/features/trace-views.md` "Folded start and end").
  const startsShown = new Set<string>();
  const endsShown = new Set<string>();
  for (const name of scope) {
    if (!inScope(name)) {
      continue;
    }
    const shown = shownAs(name);
    if (shown !== name) {
      holds.set(shown, (holds.get(shown) ?? 0) + 1);
    }
    if (starts.includes(name)) {
      startsShown.add(shown);
    }
    if (flow.ends.includes(name)) {
      endsShown.add(shown);
    }
    if (named.has(name)) {
      gaps.set(shown, (gaps.get(shown) ?? 0) + 1);
    }
    const reached = distance.get(name);
    if (reached !== undefined) {
      least.set(shown, Math.min(least.get(shown) ?? Infinity, reached));
    }
  }

  const inner = new Map<string, Map<string, number>>();
  const merged = new Map<string, FlowEdge>();
  for (const link of links) {
    const from = shownAs(link.from);
    const to = shownAs(link.to);
    if (from === to && link.from !== link.to) {
      const types = inner.get(from) ?? new Map<string, number>();
      types.set(link.type, (types.get(link.type) ?? 0) + 1);
      inner.set(from, types);
      continue;
    }
    if (from === to && from !== link.from) {
      continue;
    }
    const key = `${from}\n${to}\n${link.type}\n${link.kind ?? ""}`;
    const known = merged.get(key);
    if (known !== undefined) {
      known.count += 1;
      known.when = null;
    } else {
      merged.set(key, {
        from,
        to,
        type: link.type,
        kind: link.kind ?? null,
        when: link.when ?? null,
        count: 1,
        path: byName.get(link.type === "follows" ? link.to : link.from)?.path ?? "",
        line: 1,
      });
    }
  }

  const shown: FlowNode[] = [...scope]
    .filter((name) => inScope(name) && shownAs(name) === name)
    .flatMap((name) => {
      const document = byName.get(name);
      if (document === undefined) {
        return [];
      }
      const parent = document.parent;
      const types: FlowInner[] = [...(inner.get(name) ?? new Map<string, number>()).entries()].map(([type, count]) => ({ type, count })).sort((a, b) => byText(a.type, b.type));
      return [
        {
          id: document.id,
          kind: document.kind,
          title: document.title,
          path: document.path,
          line: document.line,
          distance: least.get(name) ?? null,
          group: parent !== null && inScope(parent) ? parent : null,
          stage: document.stage,
          start: startsShown.has(name),
          end: endsShown.has(name),
          holds: holds.get(name) ?? 0,
          inner: types,
          gaps: gaps.get(name) ?? 0,
          rollup: null,
          counts: null,
        },
      ];
    })
    .sort((a, b) => (a.distance ?? Infinity) - (b.distance ?? Infinity) || byText(a.path, b.path));
  const folded = [...merged.values()].sort((a, b) => byText(a.from, b.from) || byText(a.to, b.to) || byText(a.type, b.type) || byText(a.kind ?? "", b.kind ?? ""));
  const isStep = (node: FlowNode) => node.holds === 0 && steps.has(node.id ?? node.path);
  const kept = options.stage === undefined ? shown : shown.filter((node) => (options.stage === NO_STAGE ? node.stage === null && isStep(node) : node.stage === options.stage));
  const keptNames = new Set(kept.map((node) => node.id ?? node.path));
  const edges = options.stage === undefined ? folded : folded.filter((edge) => keptNames.has(edge.from) && keptNames.has(edge.to));
  const nodes = kept;
  return {
    root: options.root ?? null,
    depth: options.depth ?? null,
    notes: [],
    left_out: flow.leftOut,
    truncated: false,
    omitted_nodes: 0,
    omitted_edges: 0,
    gaps: rows,
    nodes,
    edges,
  };
}

/** A project's flow for a scenario, its documents and links as written. */
export function mockFlowOf(slug: string, corpus: MockCorpus, scenario: Scenario): MockFlow {
  return slug === "harbor-sim" ? harborFlow(corpus, scenario) : ledgerFlow(corpus, scenario);
}

/** The mock's flow read; a ROOT naming nothing answers the exit-1 document, as data; a bad depth is refused 503. */
export function flowOf(slug: string, corpus: MockCorpus, scenario: Scenario, options: FlowOptions): FlowView {
  if (options.depth !== undefined && (!Number.isInteger(options.depth) || options.depth < 0)) {
    throw new ClientError({ status: 503, message: `spec: --depth ${String(options.depth)}: the depth is an integer of 0 or more` });
  }
  const flow = mockFlowOf(slug, corpus, scenario);
  if (options.root !== undefined && !flow.documents.some((document) => document.name === options.root)) {
    const resolved = resolveRef(corpus, options.root);
    const reason = "reason" in resolved ? resolved.reason : `${options.root} is no document of this flow`;
    return { ...readFlow({ ...flow, documents: [], links: [] }, {}), root: options.root, reason, gaps: [], nodes: [], edges: [] };
  }
  return { reason: null, ...readFlow(flow, options) };
}
