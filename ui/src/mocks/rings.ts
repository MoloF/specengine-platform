import type { Ring, RingsDocument } from "../api/types";
import type { MockCorpus } from "./build";
import { treeOf } from "./corpus";
import type { Scenario } from "./scenario";

// The rings as the daemon's `GET .../rings` serves them
// (docs/features/scope-rings.md "Acceptance criteria", SR7, SR9): each ring the tree's live lines
// for its root (none: the tree's roots) and its kinds (none: every kind), in the tree's order, less
// a node an inner ring holds, as the core reads them; each sector's state stored by hand in turn,
// every rollup and one word this build does not know. harbor-sim: four rings, innermost first, of
// 12, 5, 1 and 0 sectors, the long-titled harbour log among the first's, the Cyrillic basin and the
// hostile night rule among the second's, the pilot station's mechanics left to the first.
// ledger-api: a bad table, its note verbatim and no ring. `empty`: no ring and no note. Every word
// is invented.

/** A `[[map.ring]]` row as the config writes it. */
interface RingRow {
  title: string;
  root: string | null;
  kinds: string[] | null;
}

/** harbor-sim's rings, innermost first. */
const HARBOR_RINGS: readonly RingRow[] = [
  { title: "Every mechanic", root: null, kinds: ["mechanic"] },
  { title: "Berths", root: "DOM-BERTHS", kinds: ["domain", "rule"] },
  { title: "Pilot station", root: "DOM-PILOTS", kinds: null },
  { title: "Locks", root: "MEC-LOCK-A", kinds: ["rule"] },
];

/** The states each sector takes in turn: the five rollups and one word this build does not know. */
const STATES = ["in work", "no card", "confirmed", "suspect", "departed", "parked"] as const;

/** ledger-api's bad table, as the daemon words it: no ring is served. */
export const LEDGER_RING_NOTE = "specengine.toml:31: `[[map.ring]]` `Refunds` holds neither `root` nor `kinds`: a ring holds one or both; no ring is served";

/** The rings of `rows` over the corpus, as the core reads them. */
function ringsOf(corpus: MockCorpus, rows: readonly RingRow[]): Ring[] {
  const held = new Set<string>();
  let turn = 0;
  return rows.map((row) => {
    const tree = treeOf(corpus, { ...(row.root === null ? {} : { root: row.root }), ...(row.kinds === null ? {} : { kinds: row.kinds }) });
    const sectors = tree.nodes
      .filter((node) => !node.archived && !held.has(`${node.path}:${String(node.line)}`))
      .map((node) => {
        held.add(`${node.path}:${String(node.line)}`);
        const state = STATES[turn % STATES.length] ?? "no card";
        turn += 1;
        return { name: node.id ?? node.path, id: node.id, kind: node.kind, title: node.title, path: node.path, line: node.line, status: node.status, state };
      });
    return { title: row.title, root: row.root, kinds: row.kinds === null ? null : [...row.kinds], sectors };
  });
}

/** A project's rings in a scenario. */
export function mockRings(corpus: MockCorpus, slug: string, scenario: Scenario): RingsDocument {
  if (scenario === "empty") {
    return { notes: [], rings: [] };
  }
  if (slug === "harbor-sim") {
    return { notes: [], rings: ringsOf(corpus, HARBOR_RINGS) };
  }
  return { notes: [LEDGER_RING_NOTE], rings: [] };
}
