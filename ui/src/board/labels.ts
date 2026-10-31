import type { CardAllowed, CardFlow, CardMove, TraceCard } from "../api/types";
import type { Look } from "../ui/Badge";

// The Board's words, one map (`docs/features/board-ui.md` "Rules and edge cases"): columns, flows
// and disciplines are project vocabulary, shown as stored; an event's type is told apart here alone.

export const ACCEPTED: Look = { label: "Accepted", tone: "card-accepted", icon: "taskAccepted" };
export const CANCELLED: Look = { label: "Cancelled", tone: "card-cancelled", icon: "taskCancelled" };
export const OVERDUE: Look = { label: "Overdue", tone: "card-overdue", icon: "cardOverdue" };

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

/** How each mark that says something looks, and its sentence (`docs/features/card-trace.md` "Rollup"). */
const MARKS: ReadonlyMap<string, { look: Look; line: (mark: TraceCard) => string }> = new Map([
  [
    "suspect",
    {
      look: { label: "Suspect", tone: "card-suspect", icon: "specChanged" },
      line: ({ on }: TraceCard) => `The brief changed at ${on ?? ""} after this card last read it.`,
    },
  ],
  [
    "departed",
    {
      look: { label: "Departed", tone: "card-departed", icon: "cardDeparted" },
      line: ({ on, discrepancy }: TraceCard) => `Accepted as departing from ${on ?? ""}; ${discrepancy ?? "its discrepancy"} is not settled yet.`,
    },
  ],
  [
    "correcting",
    {
      look: { label: "Correcting", tone: "card-correcting", icon: "taskDraft" },
      line: ({ on, correction }: TraceCard) => `The brief changed at ${on ?? ""}; ${correction ?? "a correction"} is putting it right.`,
    },
  ],
] satisfies [string, { look: Look; line: (mark: TraceCard) => string }][]);

/** Whether a mark says anything: a mark on no target is a plain state, and shows nothing. */
export function isMarked({ on }: TraceCard): boolean {
  return on !== null;
}

/** How a mark looks; one this build does not know raw and neutral; null for one that says nothing. */
export function markLook(card: TraceCard): Look | null {
  if (!isMarked(card)) {
    return null;
  }
  return MARKS.get(card.mark)?.look ?? { label: card.mark, tone: "card-mark-unknown", icon: "unknown" };
}

/** A mark in a sentence; one this build does not know, its target. */
export function markLine(card: TraceCard): string {
  return MARKS.get(card.mark)?.line(card) ?? card.on ?? "";
}

/** Whether a person can answer the mark: only a suspect one clears, a correcting one is held. */
export function canClear({ mark }: TraceCard): boolean {
  return mark === "suspect";
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
