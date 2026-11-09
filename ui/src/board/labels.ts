import type { CardAllowed, CardFlow, CardMove, CardRow, KnownTeamAct, TraceCard } from "../api/types";
import type { Translate } from "../i18n";
import type { Look } from "../ui/Badge";

// The Board's words, one map (`docs/features/board-ui.md` "Rules and edge cases"), each from the
// catalog's `board` group: columns, flows and disciplines are project vocabulary, shown as stored; an
// event's type is told apart here alone.

export function acceptedLook(t: Translate): Look {
  return { label: t("board.accepted"), tone: "card-accepted", icon: "taskAccepted" };
}

export function cancelledLook(t: Translate): Look {
  return { label: t("board.cancelled"), tone: "card-cancelled", icon: "taskCancelled" };
}

export function overdueLook(t: Translate): Look {
  return { label: t("board.overdue"), tone: "card-overdue", icon: "cardOverdue" };
}

export function openLook(t: Translate): Look {
  return { label: t("board.open"), tone: "card-open", icon: "open" };
}

export function unassigned(t: Translate): string {
  return t("board.unassigned");
}

export function byHand(t: Translate): string {
  return t("board.byHand");
}

export function notInTheFlow(t: Translate): string {
  return t("board.notInTheFlow");
}

/** The empty board's meaning and next step. */
export function noCards(t: Translate): string {
  return t("board.noCards");
}

/** How the daemon's line names the acting name: the flag `as` stands for (the daemon's words, never translated). */
export const ACTING_FLAG = "--as: ";

/** Said wherever a name is typed for an accept (ADR-0034). */
export function nameIsTyped(t: Translate): string {
  return t("board.nameIsTyped");
}

/** Said in its place on a hub: the name is the request's login, which a person cannot type over (`docs/features/hub-team.md` "Description and interactions"). */
export function nameIsLogin(t: Translate): string {
  return t("board.nameIsLogin");
}

/** The four acts a login may lack, in the table's order. */
export const TEAM_ACTS: readonly KnownTeamAct[] = ["stage", "accept", "clear", "group_clear"];

/** Said beside a control `allowed` omits. The page's words: a refusal stays the daemon's. */
export function actOff(act: KnownTeamAct, t: Translate): string {
  return t(`board.actOff.${act}`);
}

/** A card's events (`docs/canon/cards.md` "Documents and events"): each stored type is `card.` and its word here, the catalog's key. */
const EVENT_WORDS = { created: true, moved: true, accepted: true, edited: true, assigned: true, cancelled: true, cleared: true, reestimated: true, refreshed: true };

type EventWord = keyof typeof EVENT_WORDS;

const EVENT_PREFIX = "card.";

/** The nine event types, as stored. */
export const EVENT_TYPES: readonly string[] = Object.keys(EVENT_WORDS).map((word) => `${EVENT_PREFIX}${word}`);

/** An event's words; an unknown type raw. */
export function eventWords(type: string, t: Translate): string {
  const word = type.startsWith(EVENT_PREFIX) ? type.slice(EVENT_PREFIX.length) : null;
  return word !== null && Object.hasOwn(EVENT_WORDS, word) ? t(`board.event.${word as EventWord}`) : type;
}

/** A mark as a read tells it, `spec trace`'s row and the card list's alike. */
export type CardMark = Pick<TraceCard, "mark" | "on" | "discrepancy" | "correction">;

type MarkWord = "suspect" | "departed" | "correcting";

/** Each mark that says something: how it looks, its sentence, and which of its keys a tile names (`docs/features/card-trace.md` "Rollup"). */
const MARKS: Readonly<Record<MarkWord, { tone: string; icon: Look["icon"]; line: (mark: CardMark, t: Translate) => string; names: "on" | "discrepancy" | "correction" }>> = {
  suspect: {
    tone: "card-suspect",
    icon: "specChanged",
    line: ({ on }, t) => (on === null ? t("board.markLine.suspect") : t("board.markLine.suspectAt", { on })),
    names: "on",
  },
  departed: {
    tone: "card-departed",
    icon: "cardDeparted",
    line: ({ on, discrepancy }, t) =>
      t("board.markLine.departed", { on: on ?? t("board.markLine.itsBrief"), discrepancy: discrepancy ?? t("board.markLine.itsDiscrepancy") }),
    names: "discrepancy",
  },
  correcting: {
    tone: "card-correcting",
    icon: "taskDraft",
    line: ({ on, correction }, t) => {
      const by = correction ?? t("board.markLine.aCorrection");
      return on === null ? t("board.markLine.correcting", { correction: by }) : t("board.markLine.correctingAt", { on, correction: by });
    },
    names: "correction",
  },
};

/** The three marks that say something, as stored. */
export const MARK_WORDS = Object.keys(MARKS) as readonly MarkWord[];

function markOf(word: string): (typeof MARKS)[MarkWord] | undefined {
  return Object.hasOwn(MARKS, word) ? MARKS[word as MarkWord] : undefined;
}

/** The two marks that are plain states, by their word. */
const PLAIN_MARKS = { open: openLook, accepted: acceptedLook };

function plainLook(word: string, t: Translate): Look | null {
  return Object.hasOwn(PLAIN_MARKS, word) ? PLAIN_MARKS[word as keyof typeof PLAIN_MARKS](t) : null;
}

/** Whether a mark says anything against the brief: all but the two plain states, a word this build does not know among them. */
export function isMarked({ mark }: Pick<TraceCard, "mark">): boolean {
  return !Object.hasOwn(PLAIN_MARKS, mark);
}

/** How a mark looks; one this build does not know raw and neutral; null for one that says nothing. */
export function markLook(card: CardMark, t: Translate): Look | null {
  if (!isMarked(card)) {
    return null;
  }
  const known = markOf(card.mark);
  return known === undefined ? { label: card.mark, tone: "card-mark-unknown", icon: "unknown", served: true } : { label: t(`board.mark.${card.mark as MarkWord}`), tone: known.tone, icon: known.icon };
}

/** A card's word in a node's block: its mark, or the plain state its word names. */
export function cardStateLook(card: CardMark, t: Translate): Look {
  return markLook(card, t) ?? plainLook(card.mark, t) ?? { label: card.mark, tone: "card-mark-unknown", icon: "unknown", served: true };
}

/** A mark in a sentence; one this build does not know, its target, else its word. */
export function markLine(card: CardMark, t: Translate): string {
  return markOf(card.mark)?.line(card, t) ?? card.on ?? card.mark;
}

/** What a tile names beside the mark: the target, the discrepancy or the correction; one this build does not know, its target. */
export function markNames(card: CardMark): string | null {
  const key = markOf(card.mark)?.names ?? "on";
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
const GAP_WORDS = ["contradicts", "partial", "missing", "unrequested"] as const;

/** The gaps a departing accept can name, with their labels; the first is the daemon's default. */
export function gapTypes(t: Translate): readonly { word: string; label: string }[] {
  return GAP_WORDS.map((word) => ({ word, label: t(`board.gapType.${word}`) }));
}

/** The evidence kinds `spec card edit --evidence` takes. */
const EVIDENCE_WORDS = ["url", "path", "build", "note"] as const;

/** The evidence kinds, each with what to write. */
export function evidenceKinds(t: Translate): readonly { word: string; label: string; hint: string }[] {
  return EVIDENCE_WORDS.map((word) => ({ word, label: t(`board.evidence.${word}.label`), hint: t(`board.evidence.${word}.hint`) }));
}

/** Whether a note of the list names this place: `<n> card(s) in <flow>/<column>, from <first ID>: ...`. */
export function noteNames(note: string, flow: string, column: string): boolean {
  return note.includes(` in ${flow}/${column}, `);
}

/** A move's `by`: "needs <names>", any of them; null when anyone may. */
export function needsLine(by: readonly string[], t: Translate): string | null {
  return by.length === 0 ? null : t("board.needs", { names: by.join(t("board.or")) });
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
