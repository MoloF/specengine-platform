import type { CardAllowed, CardFlow, CardMove } from "../api/types";
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
]);

/** An event's words; an unknown type raw. */
export function eventWords(type: string): string {
  return EVENT_WORDS.get(type) ?? type;
}

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
