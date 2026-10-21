import type { DriftRow, HealthDrift, HealthReport } from "../api/types";
import { codeRowLook, driftClassOf, type DriftClassCounts } from "../health/labels";
import { namesOf } from "./matching";
import type { TreeRow } from "./rows";

// The tree's drift badges (docs/features/ui-tree-drift.md "Matching"): Health's row 6 rows
// (`GET …/health`, `drift.rows`; docs/canon/health-drift.md "Document") counted per tree line by
// class, matched as the inbox counter matches a target (`matching.ts` `namesOf`: the ID,
// `<stem>/<ID>`, an ID-less document's path). A line counts its node's own rows, never a
// section's (Q-D). Visibility only: nothing waits on a count (ADR-0012, ADR-0047). Class words are
// told apart in `health/labels.ts` alone. Also the "Show drift" toggle's storage ("Toggle").

/** A tree line's own row 6 rows by class. */
export type NodeDrift = Readonly<DriftClassCounts>;

/** Row 6 as the tree draws it. */
export interface TreeDrift {
  /**
   * Per tree line (`TreeRow.key`), the node's own rows by class; a line with none is absent.
   * Counts equal to the previous answer's keep its object, so a new answer renders again only the
   * lines whose counts changed; with every line unchanged, the previous map itself, so the tree
   * renders nothing again.
   */
  byRow: ReadonlyMap<string, NodeDrift>;
  /**
   * The rows naming no line of the tree read: a bare marker's `""`, an unresolved ID, a look-alike
   * as written, a node outside the tree read (another root, a cut, the archive left out).
   */
  unmatched: number;
}

// An answer is read defensively: whatever its types say, a field missing or of another shape
// leaves the drift line saying so and the tree drawn, never a thrown render.

/** A JSON value's field by name: undefined unless the value is an object (not an array). */
function fieldOf(value: unknown, name: string): unknown {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (Reflect.get(value, name) as unknown) : undefined;
}

/** A health answer's row 6, if the answer holds one at all. */
export function driftOf(report: HealthReport): HealthDrift | undefined {
  return fieldOf(report, "drift") as HealthDrift | undefined;
}

/**
 * Row 6's rows when it was measured: its state says so, its counts are an object and its rows an
 * array; else null (off, cannot, a word not listed, no counts, a malformed or missing row 6).
 */
export function measuredRows(drift: HealthDrift | undefined): DriftRow[] | null {
  const state = fieldOf(drift, "state");
  const counts = fieldOf(drift, "counts");
  const rows = fieldOf(drift, "rows");
  const measured = typeof state === "string" && codeRowLook(state).measured;
  return measured && typeof counts === "object" && counts !== null && Array.isArray(rows) ? (rows as DriftRow[]) : null;
}

/** Row 6's state and causes as sent, for the line: the state null and no cause when they are not text. */
export function driftWords(drift: HealthDrift | undefined): { state: string | null; causes: string[] } {
  const state = fieldOf(drift, "state");
  const causes = fieldOf(drift, "causes");
  return {
    state: typeof state === "string" ? state : null,
    causes: Array.isArray(causes) ? causes.filter((cause): cause is string => typeof cause === "string") : [],
  };
}

function none(): DriftClassCounts {
  return { fix: 0, check: 0, cannot: 0, unlisted: 0 };
}

function add(into: DriftClassCounts, from: NodeDrift): void {
  into.fix += from.fix;
  into.check += from.check;
  into.cannot += from.cannot;
  into.unlisted += from.unlisted;
}

function total(counts: NodeDrift): number {
  return counts.fix + counts.check + counts.cannot + counts.unlisted;
}

function same(a: NodeDrift, b: NodeDrift): boolean {
  return a.fix === b.fix && a.check === b.check && a.cannot === b.cannot && a.unlisted === b.unlisted;
}

/**
 * Row 6's rows by the `node` they name, bytes as sent: no trimming, case folding, NFC or alias
 * resolution. A `node` that is not a string is filed under `""`, which names no line; a row that is
 * not an object is skipped.
 */
function byNode(lines: readonly DriftRow[]): Map<string, DriftClassCounts> {
  const grouped = new Map<string, DriftClassCounts>();
  for (const line of lines) {
    const value: unknown = line;
    if (typeof value !== "object" || value === null) {
      continue;
    }
    const node = fieldOf(value, "node");
    const key = typeof node === "string" ? node : "";
    const counts = grouped.get(key) ?? none();
    const word = fieldOf(value, "class");
    const known = driftClassOf(typeof word === "string" ? word : null);
    if (known === null) {
      counts.unlisted += 1;
    } else {
      counts[known] += 1;
    }
    grouped.set(key, counts);
  }
  return grouped;
}

/**
 * Row 6's rows over the tree's lines: each line's own counts, the sum over its names (`namesOf`),
 * so an ID with two holders counts on both, and a section's rows never on its parent. `previous`
 * (the last answer's) lends its object to every line whose counts are unchanged, and its map when
 * no line changed, came or went. `unmatched` sums the rows whose `node` is no name of any line.
 */
export function treeDrift(rows: readonly TreeRow[], lines: readonly DriftRow[], previous: TreeDrift | null): TreeDrift {
  const grouped = byNode(lines);
  const named = new Set<string>();
  const byRow = new Map<string, NodeDrift>();
  let kept = 0;
  for (const row of rows) {
    let own: DriftClassCounts | null = null;
    for (const name of namesOf(row.node)) {
      if (name === "") {
        // No line is named `""`: a bare marker's rows, and those whose `node` is no string, stay unmatched.
        continue;
      }
      named.add(name);
      const counts = grouped.get(name);
      if (counts !== undefined) {
        own ??= none();
        add(own, counts);
      }
    }
    if (own !== null) {
      const before = previous?.byRow.get(row.key);
      if (before !== undefined && same(before, own)) {
        kept += 1;
        byRow.set(row.key, before);
      } else {
        byRow.set(row.key, own);
      }
    }
  }
  let unmatched = 0;
  for (const [name, counts] of grouped) {
    if (!named.has(name)) {
      unmatched += total(counts);
    }
  }
  // Every line as before (the same keys, each kept): the previous map, so the tree's props hold still.
  const unchanged = previous !== null && kept === byRow.size && byRow.size === previous.byRow.size;
  return { byRow: unchanged ? previous.byRow : byRow, unmatched };
}

/** Where the viewer's choice is kept, per project: the routed name after a fixed prefix (docs/features/ui-tree-drift.md "Toggle"). */
function toggleKey(project: string): string {
  return `specengine.tree.show-drift.${project}`;
}

/** The one value stored: on. Off removes the key. */
const ON = "on";

/**
 * Whether the viewer last turned "Show drift" on for the project: only a stored `on`. Off by
 * default, and off when the browser has no storage or refuses it (a private window, a sandboxed
 * frame, a policy): every access may throw, and the page works without it.
 */
export function recallShowDrift(project: string): boolean {
  try {
    return window.localStorage.getItem(toggleKey(project)) === ON;
  } catch {
    return false;
  }
}

/** Keeps the viewer's choice for the project; a refused write keeps it for the view only, nothing shown. */
export function rememberShowDrift(project: string, on: boolean): void {
  try {
    if (on) {
      window.localStorage.setItem(toggleKey(project), ON);
    } else {
      window.localStorage.removeItem(toggleKey(project));
    }
  } catch {
    // Storage refused or full: the choice holds in the view's state alone.
  }
}
