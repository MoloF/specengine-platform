import type { FlowEdge, FlowGap, FlowNode, FlowView, NodeView, ShownAncestor, TraceCounts, TraceStage, TreeNode } from "../api/types";
import { aFlowEdge, aFlowGap, aFlowNode, aFlowView, aNode, aTrace, aTreeNode, aTreeView, aWorkspace, aWorkspaces, tenGapRows } from "./builders";
import { stubClient } from "./stubClient";

// The reads `spec serve` gives on `fixtures/flow-aside`, the pottery workshop (RM-0094 flow-scenario),
// written down for the Flow's tests on the stub client: each read as the daemon serves it, `in_flow`,
// the cut and `not_in_flow` included, taken from `spec flow --json` on the fixture. Nothing here
// computes membership: the flags are the daemon's, copied. `times` 40 is the 40x variant, each record
// copied 40 times under the same prefixes. The stub's project is `flow-aside`.

export const ASIDE = "flow-aside";

/** The fixture's link types as the daemon serves them, in the model's order: the stub's data, never app code. */
export const ASIDE_TYPES = ["follows", "uses", "supplies"];

const NONE: TraceCounts = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

const STEPS = [
  ["WEDGE", "Wedging", "wheelwork"],
  ["THROW", "Throwing on the wheel", "wheelwork"],
  ["TRIM", "Trimming the foot", "wheelwork"],
  ["DRY", "Drying on the racks", "bisque"],
  ["BISQUE", "Bisque firing", "bisque"],
  ["GLAZE", "Dipping in glaze", "glazing"],
  ["FIRE", "Glost firing", "glazing"],
  ["SORT", "Sorting the kiln load", "crating"],
  ["MEND", "Mending a cracked pot", "crating"],
  ["PACK", "Packing the crates", "crating"],
] as const;

/** The parts in the flow, the gallery and the kiln among them. */
const PARTS_IN = [
  ["CLAY", "Clay cellar"],
  ["DRYING", "Drying loft"],
  ["GALLERY", "Gallery bay"],
  ["GLAZE", "Glaze kitchen"],
  ["KILN", "Kiln bay"],
  ["PACKING", "Packing shed"],
  ["SORTING", "Sorting bench"],
  ["STORE", "Finished-ware store"],
  ["WHEEL", "Wheel room"],
  ["YARD", "Loading yard"],
] as const;

/** The parts holding nothing of the flow. */
const PARTS_OUT = [
  ["CANTEEN", "Staff canteen", 0],
  ["FUEL", "Fuel shed", 0],
  ["OFFICE", "Front office", 2],
  ["WATER", "Water butts", 0],
] as const;

const NOTES = ["studio lighting", "the window sills", "visitors' boots", "the noticeboard", "the radio", "the swept floor", "the coat pegs", "the spare keys"];

export const OPEN_DAY = "docs/workshop/gallery/open-day.md";

/** The step distances `spec flow` gives: MEND and PACK share 8. */
const DISTANCE: Readonly<Record<string, number>> = { WEDGE: 0, THROW: 1, TRIM: 2, DRY: 3, BISQUE: 4, GLAZE: 5, FIRE: 6, SORT: 7, MEND: 8, PACK: 8 };

const part = (word: string, title: string, fields: Partial<FlowNode> = {}) =>
  aFlowNode({ id: `WBAY-${word}`, kind: "workbay", title, path: `docs/workshop/bays/${word.toLowerCase()}.md`, distance: null, group: "ATL-POTTERY", ...fields });

/** The rows the daemon measures over the whole flow: two starts, no `[[flow.definition]]`. */
export function asideRows(): FlowGap[] {
  const definition = "no [[flow.definition]] rule: add one naming what steps share, by paths or kinds";
  return tenGapRows([
    aFlowGap("two-starts", [["POT-WEDGE", OPEN_DAY]], { cause: "more than one start: narrow [[flow.start]] to one" }),
    aFlowGap("definition-unused", [], { state: "not_configured", count: null, cause: definition }),
    aFlowGap("definition-once", [], { state: "not_configured", count: null, cause: definition }),
  ]);
}

/** A scope with no start of its own, as the daemon measures it. */
function noStartRows(root: string): FlowGap[] {
  const definition = "no [[flow.definition]] rule: add one naming what steps share, by paths or kinds";
  return tenGapRows([
    aFlowGap("no-start", [[root]], { cause: "no step here is a start: match the flow's first step in [[flow.start]], or, reading a part, add a follows link into it from outside" }),
    { code: "unreachable", state: "cannot", count: null, items: [], cause: "no start and no entering edge to walk from" },
    aFlowGap("definition-unused", [], { state: "not_configured", count: null, cause: definition }),
    aFlowGap("definition-once", [], { state: "not_configured", count: null, cause: definition }),
  ]);
}

/** The read's own fields beside its nodes and edges. */
function read(nodes: FlowNode[], edges: FlowEdge[], fields: Partial<FlowView>): FlowView {
  return aFlowView(nodes, edges, { types: ASIDE_TYPES, ...fields });
}

/** `spec flow --depth 0 --in-flow`: the pottery alone, holding the 25 documents of the flow. */
export function asideTop(): FlowView {
  return read([aFlowNode({ id: "ATL-POTTERY", kind: "atelier", title: "The pottery", path: "docs/workshop/atelier.md", distance: 0, start: true, end: true, holds: 25, gaps: 2, inner: [{ type: "follows", count: 11 }, { type: "uses", count: 15 }] })], [], { depth: 0, gaps: asideRows(), not_in_flow: 0 });
}

const firing = (fields: Partial<FlowNode> = {}) => aFlowNode({ id: "KRUN-FIRING", kind: "kilnrun", title: "Firing run", path: "docs/workshop/run/firing.md", distance: null, ...fields });

/** The lens root's nodes in the flow, served order: the gallery and the run at 0, then the rest by path. */
function lensInFlow(): FlowNode[] {
  return [
    part("GALLERY", "Gallery bay", { distance: 0, start: true, end: true, holds: 1, gaps: 1 }),
    firing({ distance: 0, group: "ATL-POTTERY", start: true, end: true, holds: 10, gaps: 1, inner: [{ type: "follows", count: 11 }] }),
    aFlowNode({ id: "ATL-POTTERY", kind: "atelier", title: "The pottery", path: "docs/workshop/atelier.md", distance: null }),
    ...PARTS_IN.filter(([word]) => word !== "GALLERY").map(([word, title]) => part(word, title, word === "KILN" ? { holds: 144 } : {})),
  ];
}

/** The nine lifted `uses` edges from the run to its parts, the kiln's five links as one. */
function lensEdges(): FlowEdge[] {
  const counts: Readonly<Record<string, number>> = { CLAY: 1, DRYING: 1, GLAZE: 1, KILN: 5, PACKING: 1, SORTING: 2, STORE: 1, WHEEL: 2, YARD: 1 };
  return Object.entries(counts).map(([word, count]) => aFlowEdge({ from: "KRUN-FIRING", to: `WBAY-${word}`, type: "uses", count, path: "docs/workshop/run/pack.md", line: 11 }));
}

/** `spec flow ATL-POTTERY --depth 1 --in-flow --trace`: 12 nodes, 14 documents left out. */
export function asideLens(): FlowView {
  return read(lensInFlow(), lensEdges(), { root: "ATL-POTTERY", depth: 1, gaps: asideRows(), not_in_flow: 14 });
}

/** The same read uncut: the four parts and the eight notes holding nothing of the flow, too. */
export function asideLensUncut(): FlowView {
  const notes = NOTES.map((subject, index) => aFlowNode({ id: null, kind: null, title: `A note on ${subject}`, path: `docs/notes/note-${String(index + 1)}.md`, distance: null, group: "ATL-POTTERY", in_flow: false }));
  const out = PARTS_OUT.map(([word, title, holds]) => part(word, title, { in_flow: false, holds }));
  const [gallery, run, pottery, ...rest] = lensInFlow();
  const later = [...notes, ...[...rest, ...out].sort((a, b) => (a.path < b.path ? -1 : 1))];
  return read([gallery, run, pottery, ...later].flatMap((node) => (node === undefined ? [] : [node])), lensEdges(), { root: "ATL-POTTERY", depth: 1, gaps: asideRows() });
}

const step = (word: string, title: string, stage: string, fields: Partial<FlowNode> = {}) =>
  aFlowNode({ id: `POT-${word}`, kind: "potstep", title, path: `docs/workshop/run/${word.toLowerCase()}.md`, distance: DISTANCE[word] ?? null, group: "KRUN-FIRING", stage, ...fields });

/** `spec flow KRUN-FIRING --depth 1 --in-flow`: the ten steps and the run, eleven `follows`. */
export function asideFiring(): FlowView {
  const nodes = [...STEPS.map(([word, title, stage]) => step(word, title, stage, { start: word === "WEDGE", end: word === "PACK" })), firing()];
  const follows = (from: string, to: string, fields: Partial<FlowEdge> = {}) => aFlowEdge({ from: `POT-${from}`, to: `POT-${to}`, type: "follows", ...fields });
  const edges = [
    follows("BISQUE", "GLAZE"),
    follows("DRY", "BISQUE"),
    follows("FIRE", "GLAZE", { kind: "repeat" }),
    follows("FIRE", "SORT"),
    follows("GLAZE", "FIRE"),
    follows("MEND", "PACK"),
    follows("SORT", "MEND", { kind: "alternative", when: "cracked" }),
    follows("SORT", "PACK"),
    follows("THROW", "TRIM"),
    follows("TRIM", "DRY"),
    follows("WEDGE", "THROW"),
  ];
  return read(nodes, edges, { root: "KRUN-FIRING", depth: 1, not_in_flow: 0, gaps: tenGapRows(asideRows().filter((row) => row.state === "not_configured")) });
}

/** A record of the kiln bay: its prefix, kind and number. */
function record(prefix: string, kind: string, index: number): FlowNode {
  const id = `${prefix}-${String(index).padStart(4, "0")}`;
  return aFlowNode({ id, kind, title: id === "HUNCH-0007" ? "Glaze crawls on damp bisque" : `Kiln ${kind} no. ${String(index)}`, path: `docs/${kind === "hunch" ? "hunches" : `${kind}s`}/${id}.md`, distance: null, group: "WBAY-KILN", in_flow: id === "HUNCH-0007" });
}

const kiln = () => part("KILN", "Kiln bay", { group: null });
const cones = () => [
  aFlowNode({ id: "CONE-BISQUE", kind: "conerule", title: "Bisque cone", path: "docs/workshop/cones/bisque.md", distance: null, group: "WBAY-KILN" }),
  aFlowNode({ id: "CONE-GLOST", kind: "conerule", title: "Glost cone", path: "docs/workshop/cones/glost.md", distance: null, group: "WBAY-KILN" }),
];

/** `spec flow WBAY-KILN --depth 1 --in-flow`: the hunch, the bay, its two cones; 141 left out (5 679 at 40x). */
export function asideKiln(times = 1): FlowView {
  return read([record("HUNCH", "hunch", 7), kiln(), ...cones()], [], { root: "WBAY-KILN", depth: 1, gaps: noStartRows("WBAY-KILN"), not_in_flow: 142 * times - 1 });
}

/** The same read uncut: every record first, as the paths sort. */
export function asideKilnUncut(times = 1): FlowView {
  const records = [
    ...Array.from({ length: 32 * times }, (_, index) => record("EDICT", "edict", index + 1)),
    ...Array.from({ length: 60 * times }, (_, index) => record("HUNCH", "hunch", index + 1)),
    ...Array.from({ length: 50 * times }, (_, index) => record("PUZZLE", "puzzle", index + 1)),
  ];
  return read([...records, kiln(), ...cones()], [], { root: "WBAY-KILN", depth: 1, gaps: noStartRows("WBAY-KILN") });
}

/** `spec flow WBAY-OFFICE --depth 1 --in-flow`: the office alone, holding nothing of the flow; its two desk papers left out. */
export function asideOffice(): FlowView {
  return read([part("OFFICE", "Front office", { group: null, in_flow: false })], [], { root: "WBAY-OFFICE", depth: 1, gaps: noStartRows("WBAY-OFFICE"), not_in_flow: 2 });
}

/** `spec flow CONE-GLOST --depth 1 --in-flow`: the cone alone, nothing inside. */
export function asideCone(): FlowView {
  return read([aFlowNode({ id: "CONE-GLOST", kind: "conerule", title: "Glost cone", path: "docs/workshop/cones/glost.md", distance: null })], [], { root: "CONE-GLOST", depth: 1, gaps: noStartRows("CONE-GLOST"), not_in_flow: 0 });
}

const ANCESTOR: Readonly<Record<string, ShownAncestor>> = {
  "ATL-POTTERY": { id: "ATL-POTTERY", kind: "atelier", title: "The pottery", path: "docs/workshop/atelier.md" },
  "KRUN-FIRING": { id: "KRUN-FIRING", kind: "kilnrun", title: "Firing run", path: "docs/workshop/run/firing.md" },
  "WBAY-KILN": { id: "WBAY-KILN", kind: "workbay", title: "Kiln bay", path: "docs/workshop/bays/kiln.md" },
  "WBAY-GALLERY": { id: "WBAY-GALLERY", kind: "workbay", title: "Gallery bay", path: "docs/workshop/bays/gallery.md" },
  "WBAY-OFFICE": { id: "WBAY-OFFICE", kind: "workbay", title: "Front office", path: "docs/workshop/bays/office.md" },
};

/** `spec show REF --links --json`'s `ancestors` by REF, as the daemon serves them, top first. */
const CHAINS: Readonly<Record<string, readonly string[]>> = {
  "ATL-POTTERY": [],
  "KRUN-FIRING": ["ATL-POTTERY"],
  "WBAY-KILN": ["ATL-POTTERY"],
  "WBAY-OFFICE": ["ATL-POTTERY"],
  "WBAY-GALLERY": ["ATL-POTTERY"],
  "CONE-GLOST": ["ATL-POTTERY", "WBAY-KILN"],
  "CONE-BISQUE": ["ATL-POTTERY", "WBAY-KILN"],
  "HUNCH-0007": ["ATL-POTTERY", "WBAY-KILN"],
  "POT-WEDGE": ["ATL-POTTERY", "KRUN-FIRING"],
  "POT-FIRE": ["ATL-POTTERY", "KRUN-FIRING"],
};

/** A node read: its title and kind as the flow reads give them, its `ancestors`; a REF the fixture lacks, the exit-1 document. */
export function asideNode(ref: string): NodeView {
  const held = [asideTop(), asideLensUncut(), asideFiring(), asideKilnUncut(), asideOffice()].flatMap((view) => view.nodes).find((node) => (node.id ?? node.path) === ref);
  if (held === undefined) {
    return { ref, reason: `\`${ref}\` resolves to no ID and no alias`, notes: [], nodes: [] };
  }
  // The CLI's chains: a root none; a part, a note and the run under the pottery; a record, a cone or a desk paper under its bay too.
  const fallback = held.group === null ? [] : held.group === "ATL-POTTERY" ? ["ATL-POTTERY"] : ["ATL-POTTERY", held.group];
  const ancestors = (CHAINS[ref] ?? fallback).flatMap((name) => (ANCESTOR[name] === undefined ? [] : [ANCESTOR[name]]));
  return { ref, reason: null, notes: [], nodes: [aNode({ id: held.id, kind: held.kind, title: held.title, path: held.path, ancestors, links: { outgoing: [], incoming: [], omitted: 0, left_out: { generated: 0, tier3: 0 } } })] };
}

/** The lanes `spec trace REF --summary` gives: the run's four stages; none elsewhere. */
function asideLanes(ref: string): TraceStage[] {
  if (ref !== "KRUN-FIRING" && ref !== "ATL-POTTERY") {
    return [];
  }
  return [
    { stage: "wheelwork", distance: 0, counts: { ...NONE, no_card: 3 } },
    { stage: "bisque", distance: 3, counts: { ...NONE, no_card: 2 } },
    { stage: "glazing", distance: 5, counts: { ...NONE, no_card: 2 } },
    { stage: "crating", distance: 7, counts: { ...NONE, no_card: 3 } },
  ];
}

/** The tree's rows the seek and the gap titles read: every document of the flow, its parent by name. */
export function asideTree(): TreeNode[] {
  const rows = [asideTop(), asideLensUncut(), asideFiring(), asideKilnUncut(), asideOffice()].flatMap((view) => view.nodes);
  const seen = new Set<string>();
  return [
    ...rows.flatMap((node) => {
      const name = node.id ?? node.path;
      if (seen.has(name)) {
        return [];
      }
      seen.add(name);
      return [aTreeNode({ id: node.id, title: node.title, path: node.path, depth: node.group === null ? 0 : 1, parent: name === "ATL-POTTERY" ? null : (node.group ?? "ATL-POTTERY") })];
    }),
    aTreeNode({ id: null, title: "Open day", path: OPEN_DAY, depth: 2, parent: "WBAY-GALLERY" }),
  ];
}

/** Each drawing read by its form; another form or REF, the exit-1 document. */
function flowRead(options: { root?: string; depth?: number; stage?: string; in_flow?: boolean }, times: number): FlowView {
  const cut = options.in_flow === true;
  if (options.root === undefined) {
    return asideTop();
  }
  if (options.stage !== undefined) {
    const whole = asideFiring();
    const nodes = whole.nodes.filter((node) => node.stage === options.stage);
    const kept = new Set(nodes.map((node) => node.id));
    return { ...whole, depth: null, nodes, edges: whole.edges.filter((edge) => kept.has(edge.from) && kept.has(edge.to)) };
  }
  switch (options.root) {
    case "ATL-POTTERY":
      return cut ? asideLens() : asideLensUncut();
    case "KRUN-FIRING":
      return asideFiring();
    case "WBAY-KILN":
      return cut ? asideKiln(times) : asideKilnUncut(times);
    case "WBAY-OFFICE":
      return cut ? asideOffice() : read([...asideOffice().nodes, ...["LEDGER", "ROTA"].map((word) => aFlowNode({ id: `DESK-${word}`, kind: "deskpaper", title: word === "LEDGER" ? "Order ledger" : "Weekly rota", path: `docs/workshop/office/${word.toLowerCase()}.md`, distance: null, group: "WBAY-OFFICE", in_flow: false }))], [], { root: "WBAY-OFFICE", depth: 1, gaps: noStartRows("WBAY-OFFICE") });
    case "CONE-GLOST":
      return asideCone();
    default:
      return read([], [], { root: options.root, depth: 1, reason: `\`${options.root}\` resolves to no ID and no alias`, gaps: [] });
  }
}

/**
 * A stub client answering Flow's reads on the pottery workshop as `spec serve` does: the cut forms
 * and the uncut Records read, node reads with `ancestors`, the run's lanes, the tree; `module` serves
 * one `[[module]]` rooted there.
 */
export function asideClient({ times = 1, module = null }: { times?: number; module?: string | null } = {}) {
  const client = stubClient();
  client.getProjects.mockResolvedValue([{ slug: ASIDE, name: "Flow aside", root: "/work/flow-aside", branch: "main" }]);
  client.getFlow.mockImplementation((_project, options) => Promise.resolve(structuredClone(flowRead(options, times))));
  client.getNode.mockImplementation((_project, ref) => Promise.resolve(structuredClone(asideNode(ref))));
  client.getTrace.mockImplementation((_project, options) => Promise.resolve(aTrace([], { ref: options?.ref ?? null, stages: asideLanes(options?.ref ?? "") })));
  client.getTree.mockImplementation(() => Promise.resolve(aTreeView(asideTree())));
  if (module !== null) {
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "all", title: "All sections" })], [{ name: "kiln", title: "Kiln bay", root: module }]));
  }
  return client;
}
