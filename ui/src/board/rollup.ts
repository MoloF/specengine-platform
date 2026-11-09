import type { CardEvent, CardFlow, CardRow } from "../api/types";
import { handOffWord, isReturned, type HandOffWord } from "./labels";

// A card's hand-offs (`docs/canon/card-flows.md` "The rule"): its children are the rows of the one
// list read whose `parent` is its ID, a cancelled one left out. Each child's word is looked up in
// labels.ts; nothing here judges a column, and a parent waits on no child.

/** One child as its parent's rollup shows it: the row and its word, null in a flow not served. */
export interface HandOff {
  row: CardRow;
  word: HandOffWord | null;
}

/** A parent's rollup: its children in the list's order, and how many of them are returned. */
export interface Rollup {
  children: HandOff[];
  returned: number;
}

/** Every parent's rollup from one list, so a board of thousands reads its rows once. A child of a flow not served still counts. */
export function rollupsOf(rows: readonly CardRow[], flows: readonly CardFlow[]): ReadonlyMap<string, Rollup> {
  const byName = new Map(flows.map((flow) => [flow.name, flow] as const));
  const found = new Map<string, { children: HandOff[]; returned: number }>();
  for (const row of rows) {
    if (row.parent === null || row.cancelled) {
      continue;
    }
    const word = handOffWord(row, byName.get(row.flow));
    const rollup = found.get(row.parent) ?? { children: [], returned: 0 };
    rollup.children.push({ row, word });
    rollup.returned += isReturned(word) ? 1 : 0;
    found.set(row.parent, rollup);
  }
  return found;
}

/** Why a card came back: a move's note, who moved it and when. */
export interface ReturnReason {
  note: string;
  by: string | null;
  at: string;
}

/**
 * The latest reason a card's history keeps for entering `column`: the last entry whose payload's `to`
 * is that column and whose `note` is text, whatever its type, so no event type is named. Null once
 * the history no longer holds one (it keeps the last 64 events).
 */
export function returnReasonOf(history: readonly CardEvent[], column: string): ReturnReason | null {
  for (const event of [...history].sort((a, b) => b.seq - a.seq)) {
    if (typeof event.payload !== "object" || event.payload === null) {
      continue;
    }
    const { to, note, as } = event.payload as Record<string, unknown>;
    if (to === column && typeof note === "string") {
      return { note, by: typeof as === "string" ? as : null, at: event.at };
    }
  }
  return null;
}
