import type { HistoryEntry, ShownHistory } from "../api/types";
import { fakeHex, stamp, type MockNode } from "./build";
import { DRAFT_FILE } from "./harbor-sim/fixtures";

// The mock's `spec show --history` (docs/features/node-history.md "Acceptance criteria", NH13): a
// holder's changes as the daemon serves them, newest first, uncut (`omitted` 0), times relative to
// the mock's clock. harbor-sim: MEC-MOORING has the whole story, a hand commit creating it, a raise,
// a wording fix, its task C-0005 accepted (as the Board's card holds it), a decision recorded in
// DEC-0014 and a later hand edit; the pilots' log has 64 hand edits and more, `truncated`; DOM-HARBOR
// holds a change word no build knows; the ID-less draft file is uncommitted, `[]`; every other node
// one hand commit creating it. ledger-api is a shallow clone: every holder `null` and its note.
// Invented names; every address under `.example.invalid`.

const DAY = 24 * 60;

/** A history entry, absent keys null. */
function entry(fields: Partial<HistoryEntry> & Pick<HistoryEntry, "source" | "at" | "id" | "change" | "by" | "by_name">): HistoryEntry {
  return {
    created: false,
    rev: null,
    rev_before: null,
    proposal: null,
    proposed_by: null,
    proposer: null,
    record: null,
    title: null,
    rationale: null,
    ...fields,
  };
}

/** A commit's entry: its OID a stable fake of its place. */
function commit(at: number, minutesAgo: number, seed: string, fields: Partial<HistoryEntry> & Pick<HistoryEntry, "change" | "by" | "by_name">): HistoryEntry {
  return entry({ source: "commit", at: stamp(at, minutesAgo), id: fakeHex(`history:${seed}`, 40), ...fields });
}

const MIRA = { by: "Mira Holt <mira@harbor.example.invalid>", by_name: "Mira Holt" };
const LENA = { by: "Lena Fisk <lena@harbor.example.invalid>", by_name: "Lena Fisk" };
const TOMAS = { by: "Tomas Oakes <tomas@harbor.example.invalid>", by_name: "Tomas Oakes" };
const IVO = { by: "Ivo Brandt <ivo@harbor.example.invalid>", by_name: "Ivo Brandt" };

/** MEC-MOORING's story, newest first. C-0005's acceptance is the Board's (src/mocks/cards.ts: four days ago, by Mira Holt). */
function mooring(at: number): HistoryEntry[] {
  return [
    commit(at, DAY, "mooring:5", { change: "hand", rev: 2, rev_before: 2, title: "Fix the night mooring time", ...LENA }),
    entry({ source: "item", at: stamp(at, 2 * DAY), id: "PR-0036", change: "decision", record: "DEC-0014", ...MIRA }),
    entry({ source: "card", at: stamp(at, 4 * DAY), id: "C-0005", change: "accepted", by: "Mira Holt", by_name: "Mira Holt", title: "Mooring fees table" }),
    commit(at, 6 * DAY, "mooring:3", {
      change: "editorial",
      rev: 2,
      rev_before: 2,
      proposal: "PR-0033",
      proposed_by: "agent role=spec-writer model=claude-opus-5-5 run=r-0412",
      proposer: "agent spec-writer",
      rationale: "Wording only: a crew serves one berth at a time.",
      ...MIRA,
    }),
    commit(at, 9 * DAY, "mooring:2", {
      change: "raised",
      rev: 2,
      rev_before: 1,
      proposal: "PR-0031",
      proposed_by: "agent role=spec-writer model=claude-opus-5-5 run=r-0398",
      proposer: "agent spec-writer",
      rationale: "A free berth takes the first ship whose draft and length fit, not the first in line.",
      ...MIRA,
    }),
    commit(at, 20 * DAY, "mooring:1", { change: "hand", created: true, rev: 1, title: "Add the mooring rules", ...TOMAS }),
  ];
}

/** The pilots' log: an entry added by hand each day, 64 listed of more. */
function harbourLog(at: number): ShownHistory {
  const entries = Array.from({ length: 64 }, (_, index) =>
    commit(at, (index + 1) * 60, `log:${String(index)}`, { change: "hand", rev: 1, rev_before: 1, title: `Log entry ${String(64 - index).padStart(2, "0")}`, ...IVO }),
  );
  return { entries, truncated: true, omitted: 0 };
}

/** DOM-HARBOR: a change word a later daemon may serve, then its creation. */
function harbor(at: number): HistoryEntry[] {
  return [
    commit(at, 3 * DAY, "harbor:2", { change: "reverted", rev: 1, rev_before: 1, title: "Revert the clock rename", ...LENA }),
    commit(at, 30 * DAY, "harbor:1", { change: "hand", created: true, rev: 1, title: "Add the harbour", ...TOMAS }),
  ];
}

/** A node with no story of its own: the hand commit that added it. */
function added(at: number, node: MockNode): ShownHistory {
  const name = node.id ?? node.path;
  return { entries: [commit(at, 30 * DAY, `added:${node.path}:${name}`, { change: "hand", created: true, rev: node.rev ?? 1, title: `Add ${name}`, ...TOMAS })], truncated: false, omitted: 0 };
}

/** One project's history reads: each holder's, and the read's notes on the holders it could not read. */
export interface MockHistory {
  of(node: MockNode): ShownHistory | null;
  notes(holders: readonly MockNode[]): string[];
}

/** Not asked: nothing read, no note. */
export const NO_HISTORY: MockHistory = {
  of: () => null,
  notes: () => [],
};

/** A project's history as `spec serve` reads it in this mock, its times `at`'s. */
export function mockHistory(slug: string, at: number): MockHistory {
  if (slug !== "harbor-sim") {
    // A `--depth 1` clone: no history to read, one note per holder (NH5).
    return {
      of: () => null,
      notes: (holders) => holders.map((holder) => `history: ${holder.path}: not read: a shallow clone`),
    };
  }
  return {
    of: (node) => {
      if (node.path === DRAFT_FILE) {
        return { entries: [], truncated: false, omitted: 0 };
      }
      switch (node.id) {
        case "MEC-MOORING":
          return { entries: mooring(at), truncated: false, omitted: 0 };
        case "MEC-HARBOUR-LOG":
          return harbourLog(at);
        case "DOM-HARBOR":
          return { entries: harbor(at), truncated: false, omitted: 0 };
        default:
          return added(at, node);
      }
    },
    notes: () => [],
  };
}
