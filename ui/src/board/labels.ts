import type { CardAllowed, CardFlow, CardMove, CardRow, TraceCard } from "../api/types";
import type { Look } from "../ui/Badge";

// The Board's words, one map (`docs/features/board-ui.md` "Rules and edge cases"): columns, flows
// and disciplines are project vocabulary, shown as stored; an event's type is told apart here alone.

export const ACCEPTED: Look = { label: "Accepted", tone: "card-accepted", icon: "taskAccepted" };
export const CANCELLED: Look = { label: "Cancelled", tone: "card-cancelled", icon: "taskCancelled" };
export const OVERDUE: Look = { label: "Overdue", tone: "card-overdue", icon: "cardOverdue" };
export const OPEN: Look = { label: "Open", tone: "card-open", icon: "open" };

export const UNASSIGNED = "Unassigned";
export const BY_HAND = "By hand";
export const NOT_IN_THE_FLOW = "Not in the flow";

/** The empty board's meaning and next step. */
export const NO_CARDS = "No cards yet. Next: press New card, press New card on a node in the Spec tree, or have your AI call create_card.";

/** How the daemon's line names the acting name: the flag `as` stands for. */
export const ACTING_FLAG = "--as: ";

/** Said wherever a name is typed for an accept (ADR-0034). */
export const NAME_IS_TYPED = "This is a convention until a named login: the name is typed and nothing verifies it.";

/** The words of a card's events, by their stored type (`docs/canon/cards.md` "Documents and events"). */
const EVENT_WORDS: ReadonlyMap<string, string> = new Map([
  ["card.created", "Created"],
  ["card.moved", "Moved"],
  ["card.accepted", "Accepted"],
  ["card.edited", "Edited"],
  ["card.assigned", "Assigned"],
  ["card.cancelled", "Cancelled"],
  ["card.cleared", "Cleared"],
  ["card.reestimated", "Re-estimated"],
  ["card.refreshed", "Refreshed"],
]);

/** An event's words; an unknown type raw. */
export function eventWords(type: string): string {
  return EVENT_WORDS.get(type) ?? type;
}

/** A mark as a read tells it, `spec trace`'s row and the card list's alike. */
export type CardMark = Pick<TraceCard, "mark" | "on" | "discrepancy" | "correction">;

/** Each mark that says something: how it looks, its sentence, and which of its keys a tile names (`docs/features/card-trace.md` "Rollup"). */
const MARKS: ReadonlyMap<string, { look: Look; line: (mark: CardMark) => string; names: "on" | "discrepancy" | "correction" }> = new Map([
  [
    "suspect",
    {
      look: { label: "Suspect", tone: "card-suspect", icon: "specChanged" },
      line: ({ on }: CardMark) => (on === null ? "The brief changed after this card last read it." : `The brief changed at ${on} after this card last read it.`),
      names: "on",
    },
  ],
  [
    "departed",
    {
      look: { label: "Departed", tone: "card-departed", icon: "cardDeparted" },
      line: ({ on, discrepancy }: CardMark) => `Accepted as departing from ${on ?? "its brief"}; ${discrepancy ?? "its discrepancy"} is not settled yet.`,
      names: "discrepancy",
    },
  ],
  [
    "correcting",
    {
      look: { label: "Correcting", tone: "card-correcting", icon: "taskDraft" },
      line: ({ on, correction }: CardMark) => `The brief changed${on === null ? "" : ` at ${on}`}; ${correction ?? "a correction"} is putting it right.`,
      names: "correction",
    },
  ],
] satisfies [string, { look: Look; line: (mark: CardMark) => string; names: "on" | "discrepancy" | "correction" }][]);

/** The two marks that are plain states, by their word. */
const PLAIN_MARKS: ReadonlyMap<string, Look> = new Map(Object.entries({ open: OPEN, accepted: ACCEPTED }));

/** Whether a mark says anything against the brief: all but the two plain states, a word this build does not know among them. */
export function isMarked({ mark }: Pick<TraceCard, "mark">): boolean {
  return !PLAIN_MARKS.has(mark);
}

/** How a mark looks; one this build does not know raw and neutral; null for one that says nothing. */
export function markLook(card: CardMark): Look | null {
  if (!isMarked(card)) {
    return null;
  }
  return MARKS.get(card.mark)?.look ?? { label: card.mark, tone: "card-mark-unknown", icon: "unknown" };
}

/** A card's word in a node's block: its mark, or the plain state its word names. */
export function cardStateLook(card: CardMark): Look {
  return markLook(card) ?? PLAIN_MARKS.get(card.mark) ?? { label: card.mark, tone: "card-mark-unknown", icon: "unknown" };
}

/** A mark in a sentence; one this build does not know, its target, else its word. */
export function markLine(card: CardMark): string {
  return MARKS.get(card.mark)?.line(card) ?? card.on ?? card.mark;
}

/** What a tile names beside the mark: the target, the discrepancy or the correction; one this build does not know, its target. */
export function markNames(card: CardMark): string | null {
  const key = MARKS.get(card.mark)?.names ?? "on";
  return card[key];
}

/** Whether a person can answer the mark: only a suspect one clears, a correcting one is held. */
export function canClear({ mark }: Pick<TraceCard, "mark">): boolean {
  return mark === "suspect";
}

/** A list row's mark, the list's own, so a gone target cannot hide it; null for a read with no marks and for a plain state: a word this build does not know is a mark, never a clean tile. */
export function rowMark({ mark, on, discrepancy, correction }: Pick<CardRow, "mark" | "on" | "discrepancy" | "correction">): CardMark | null {
  return mark === null || !isMarked({ mark }) ? null : { mark, on, discrepancy, correction };
}

/** The gaps a departing accept can name; the first is the daemon's default. */
export const GAP_TYPES: readonly { word: string; label: string }[] = [
  { word: "contradicts", label: "Contradicts the brief" },
  { word: "partial", label: "Part of the brief" },
  { word: "missing", label: "Missing from the result" },
  { word: "unrequested", label: "Not in the brief" },
];

/** The evidence kinds `spec card edit --evidence` takes, each with what to write. */
export const EVIDENCE_KINDS: readonly { word: string; label: string; hint: string }[] = [
  { word: "url", label: "Link", hint: "http:// or https://; nothing is fetched" },
  { word: "path", label: "File", hint: "a relative path, @ and its blob hash" },
  { word: "build", label: "Build", hint: "a build's name" },
  { word: "note", label: "Note", hint: "one line" },
];

/** Whether a note of the list names this place: `<n> card(s) in <flow>/<column>, from <first ID>: ...`. */
export function noteNames(note: string, flow: string, column: string): boolean {
  return note.includes(` in ${flow}/${column}, `);
}

/** A move's `by`: "needs <names>", any of them; null when anyone may. */
export function needsLine(by: readonly string[]): string | null {
  return by.length === 0 ? null : `needs ${by.join(" or ")}`;
}

/** A column's title in its flow; a column the flow no longer lists raw. */
export function columnTitle(flow: CardFlow | undefined, column: string): string {
  return flow?.columns.find((candidate) => candidate.id === column)?.title ?? column;
}

/** The moves of a column as the flow lists them, in served order, shaped as `allowed`. */
export function movesFrom(flow: CardFlow | undefined, column: string): CardAllowed[] {
  const listed: readonly CardMove[] = flow?.moves ?? [];
  return listed.filter((move) => move.from === column).map(({ to, by }) => ({ to, by }));
}
