import type { CardEvent, CardFlow, CardRow } from "../api/types";
import { categoryOf, type StatusCategory } from "./category";
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

/** The bar's words, in its order: accepted, cancelled, then an open child's category (docs/features/handoff-progress.md "Acceptance criteria", HP-2). */
const PROGRESS_WORDS = { accepted: true, cancelled: true, returned: true, doing: true, todo: true, stranded: true };

/** A segment's word, in the bar's order: accepted, cancelled, then an open child's category. */
export type ProgressWord = keyof typeof PROGRESS_WORDS;

/** Each word by itself, so none is spelled twice. */
const PROGRESS = Object.fromEntries(Object.keys(PROGRESS_WORDS).map((word) => [word, word])) as { readonly [K in ProgressWord]: K };

/** The words in the bar's order. */
export const PROGRESS_ORDER = Object.keys(PROGRESS_WORDS) as readonly ProgressWord[];

/**
 * An open child's word, by its column's category: `accept.to` holds an open card only after a config
 * change, and it waits on an accept, so it counts as in progress.
 */
const OPEN_CHILD: { readonly [K in StatusCategory]: ProgressWord } = {
  todo: PROGRESS.todo,
  doing: PROGRESS.doing,
  done: PROGRESS.doing,
  returned: PROGRESS.returned,
  stranded: PROGRESS.stranded,
};

export interface ProgressSegment {
  word: ProgressWord;
  count: number;
}

export interface Progress {
  /** The children not cancelled, accepted ones too, in served order, each with its hand-off word. */
  children: HandOff[];
  /** The cancelled children, in served order. */
  cancelled: CardRow[];
  /** One per non-zero word, in ProgressWord's order; the counts sum to every child. */
  segments: ProgressSegment[];
  /** The parent open, one child or more, every child accepted or cancelled. */
  ready: boolean;
  /** On an open parent, the children neither accepted nor cancelled, in served order; else empty. */
  reliesOn: CardRow[];
}

/** A child's word on the bar: cancelled, else accepted, by its row; an open child's by its column's category, looked up. */
function progressWordOf(row: CardRow, flow: CardFlow | undefined): ProgressWord {
  if (row.cancelled) {
    return PROGRESS.cancelled;
  }
  if (row.accepted) {
    return PROGRESS.accepted;
  }
  return OPEN_CHILD[categoryOf(flow, row.column)];
}

/**
 * A card's progress from its children read (docs/features/handoff-progress.md "Acceptance criteria",
 * HP-2): the rows whose `parent` is the card, a row of another parent and the card's own ignored.
 * Every word is looked up, and nothing here closes, moves or waits on anything: Ready is a status,
 * never a gate on Accept (ADR-0074).
 */
export function progressOf(rows: readonly CardRow[], flows: readonly CardFlow[], parent: { id: string; open: boolean }): Progress {
  const byName = new Map(flows.map((flow) => [flow.name, flow] as const));
  const children: HandOff[] = [];
  const cancelled: CardRow[] = [];
  const reliesOn: CardRow[] = [];
  const counts = new Map<ProgressWord, number>();
  for (const row of rows) {
    if (row.parent !== parent.id || row.id === parent.id) {
      continue;
    }
    const flow = byName.get(row.flow);
    const word = progressWordOf(row, flow);
    counts.set(word, (counts.get(word) ?? 0) + 1);
    if (row.cancelled) {
      cancelled.push(row);
      continue;
    }
    children.push({ row, word: handOffWord(row, flow) });
    if (parent.open && !row.accepted) {
      reliesOn.push(row);
    }
  }
  const segments = PROGRESS_ORDER.flatMap((word) => {
    const count = counts.get(word) ?? 0;
    return count === 0 ? [] : [{ word, count }];
  });
  const total = children.length + cancelled.length;
  return { children, cancelled, segments, ready: parent.open && total > 0 && reliesOn.length === 0, reliesOn };
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
