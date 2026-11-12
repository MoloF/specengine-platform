import type { CardUpstream, CardUpstreamChange } from "../api/types";
import { stamp } from "./build";

// What changed above harbor-sim's open subtasks, as a card read serves it:
// docs/features/upstream-signal.md "Acceptance criteria", US-13. Fixed reads, never judged here; a
// closed card reads none. C-0013, C-0001's plate, has its parent's mark, the corpus's applied decision
// PR-0056 (its record DEC-0013, a document the mock serves) at its applied event's `seq`, then its
// parent's goal edited; C-0019, the glossary, lists sixteen changes with four older ones not listed.
// A list with marks flags both rows, as their reads say something; every other row reads `false`.

/** The rows a list with marks flags. */
export const HARBOR_FLAGGED: ReadonlySet<string> = new Set(["C-0013", "C-0019"]);

/** A parent's edit of the fields that count. */
function edit(seq: number, at: string, by: string | null, fields: string[]): CardUpstreamChange {
  return { seq, at, source: "edit", by, fields, item: null, kind: null, record: null, on: null };
}

/** An item applied on the reach, its record when it made one. */
function item(seq: number, at: string, by: string | null, kind: string, id: string, record: string | null, on: string): CardUpstreamChange {
  return { seq, at, source: "item", by, fields: [], item: id, kind, record, on };
}

/** The glossary's sixteen changes, oldest first: twelve edits and four items, two questions with no record. */
function glossaryChanges(at: number): CardUpstreamChange[] {
  const fields = [["goal"], ["criteria"], ["targets"], ["goal", "criteria"], ["goal", "targets", "criteria"]];
  const items: [string, string, string | null][] = [
    ["decision", "PR-0030", "DEC-0012"],
    ["question", "PR-0032", null],
    ["discrepancy", "PR-0034", "DEC-0014"],
    ["question", "PR-0037", null],
  ];
  return Array.from({ length: 16 }, (_, index) => {
    const seq = 40 + index * 2;
    const when = stamp(at, 16 * 60 - index * 60);
    const found = items[Math.floor(index / 4)];
    if (index % 4 === 3 && found !== undefined) {
      const [kind, id, record] = found;
      return item(seq, when, index === 7 ? null : "Ivo Brandt", kind, id, record, "DOM-WATER");
    }
    return edit(seq, when, "Mira Holt", fields[index % fields.length] ?? ["goal"]);
  });
}

/** Each open subtask's read, by card ID. */
export function harborUpstream(at: number): Map<string, CardUpstream> {
  return new Map([
    [
      "C-0013",
      {
        parent: "C-0001",
        mark: "suspect",
        on: "MEC-TIDE-TABLES",
        correction: null,
        // PR-0056's `proposal.applied` is the board's 26th event (src/mocks/cards.ts harborItemEvents), its decider Mira Holt.
        changes: [item(26, stamp(at, 540), "Mira Holt", "decision", "PR-0056", "DEC-0013", "MEC-CARGO-CRANES"), edit(31, stamp(at, 120), "Mira Holt", ["goal"])],
        more: 0,
      },
    ],
    ["C-0019", { parent: "C-0001", mark: null, on: null, correction: null, changes: glossaryChanges(at), more: 4 }],
  ]);
}
