import type { CardAllowed, CardFlow, CardMove, CardRow, KnownProposalStatus, KnownTeamAct, TraceCard } from "../api/types";
import type { Translate } from "../i18n";
import type { Look } from "../ui/Badge";
import type { IconName } from "../ui/Icon";
import type { LozengeAppearance, LozengeLook } from "../ui/Lozenge";
import type { StatusCategory } from "./category";
import type { ProgressWord } from "./rollup";

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

/** A card standing in its flow's `return` column: sent back, a label and an icon (`docs/canon/card-flows.md` "The rule"). */
export function returnedLook(t: Translate): Look {
  return { label: t("board.return.badge"), tone: "card-returned", icon: "cardReturned" };
}

/** A parent's returned children, counted: the same look, the count in its label. */
export function returnedCountLook(count: number, t: Translate): Look {
  return { label: t("board.rollup.returnedCount", { count }), tone: "card-returned", icon: "cardReturned" };
}

const HAND_OFF_LOOKS = {
  accepted: acceptedLook,
  returned: returnedLook,
  handedOff: (t: Translate): Look => ({ label: t("board.rollup.handedOff"), tone: "card-handed-off", icon: "cardHandOff" }),
  taken: (t: Translate): Look => ({ label: t("board.rollup.taken"), tone: "card-taken", icon: "taskInProgress" }),
};

/**
 * A child's word in its parent's rollup (`docs/canon/card-flows.md` "The rule"), looked up and never
 * judged: accepted, then, in a flow the read serves, returned in its `return` column, handed off in
 * its first column, taken in any other. A flow not served has no word.
 */
export type HandOffWord = keyof typeof HAND_OFF_LOOKS;

/** Each word by itself, so none is spelled twice. */
const WORDS = Object.fromEntries(Object.keys(HAND_OFF_LOOKS).map((word) => [word, word])) as { readonly [K in HandOffWord]: K };

/** A child's word, as `CardRow` and its flow say; null for a flow the read does not serve. */
export function handOffWord(row: Pick<CardRow, "column"> & { accepted: boolean }, flow: CardFlow | undefined): HandOffWord | null {
  if (row.accepted) {
    return WORDS.accepted;
  }
  if (flow === undefined) {
    return null;
  }
  if (row.column === flow.return) {
    return WORDS.returned;
  }
  return row.column === flow.columns[0]?.id ? WORDS.handedOff : WORDS.taken;
}

export function handOffLook(word: HandOffWord, t: Translate): Look {
  return HAND_OFF_LOOKS[word](t);
}

/** Whether a word is a return's: the parent's tile counts these. */
export function isReturned(word: HandOffWord | null): boolean {
  return word === WORDS.returned;
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

/** Whether this build has words for an event, a card's or an item's: one it has none for is shown raw, under `lang="en"`. */
export function knowsEvent(type: string): boolean {
  const word = type.startsWith(EVENT_PREFIX) ? type.slice(EVENT_PREFIX.length) : type.startsWith(ITEM_EVENT_PREFIX) ? type.slice(ITEM_EVENT_PREFIX.length) : null;
  return word !== null && (type.startsWith(EVENT_PREFIX) ? Object.hasOwn(EVENT_WORDS, word) : Object.hasOwn(ITEM_EVENT_WORDS, word));
}

/** An event's words; an unknown type raw. */
export function eventWords(type: string, t: Translate): string {
  const word = type.startsWith(EVENT_PREFIX) ? type.slice(EVENT_PREFIX.length) : null;
  return word !== null && Object.hasOwn(EVENT_WORDS, word) ? t(`board.event.${word as EventWord}`) : type;
}

/**
 * A card's items' events (docs/canon/card-questions.md "The rule"; docs/features/card-questions.md
 * "Acceptance criteria", U1): each stored type is `proposal.` and its word here, the catalog's key,
 * beside the card's own; the Activity names nothing else of an event.
 */
const ITEM_EVENT_WORDS = { created: true, staged: true, unstaged: true, approved: true, applied: true, rejected: true, apply_failed: true };

type ItemEventWord = keyof typeof ITEM_EVENT_WORDS;

const ITEM_EVENT_PREFIX = "proposal.";

/** The event that raised an item: its block stands under it. */
const ITEM_RAISED: ItemEventWord = "created";

/** Whether an event is one of an item's: the queue's, not the card's. */
export function isItemEvent(type: string): boolean {
  return type.startsWith(ITEM_EVENT_PREFIX);
}

/** Whether an event raised its item, whose block opens at it. */
export function raisesItem(type: string): boolean {
  return type === `${ITEM_EVENT_PREFIX}${ITEM_RAISED}`;
}

/** An item's event's words; an unknown type raw. */
export function itemEventWords(type: string, t: Translate): string {
  const word = isItemEvent(type) ? type.slice(ITEM_EVENT_PREFIX.length) : null;
  return word !== null && Object.hasOwn(ITEM_EVENT_WORDS, word) ? t(`board.activity.${word as ItemEventWord}`) : type;
}

/** How an item's state looks as a lozenge: a proposal's seven, each with its icon; any other raw and neutral. */
const ITEM_STATUSES: Readonly<Record<KnownProposalStatus, { appearance: LozengeAppearance; icon: IconName }>> = {
  open: { appearance: "neutral", icon: "open" },
  changes_requested: { appearance: "warning", icon: "changes" },
  approved: { appearance: "information", icon: "approved" },
  applied: { appearance: "success", icon: "applied" },
  rejected: { appearance: "danger", icon: "rejected" },
  deferred: { appearance: "neutral", icon: "deferred" },
  superseded: { appearance: "neutral", icon: "superseded" },
};

/** An item's state: its lozenge, labelled as the Inbox labels it; a word this build does not know raw, under `lang="en"`. */
export function itemStatusLook(status: string, t: Translate): LozengeLook & { lang?: "en" } {
  if (!Object.hasOwn(ITEM_STATUSES, status)) {
    return { appearance: "neutral", icon: "unknown", label: status, lang: "en" };
  }
  const known = status as KnownProposalStatus;
  return { ...ITEM_STATUSES[known], label: t(`inbox.status.${known}`) };
}

/** The state an item is answered in: the one Answer… stages on (`docs/canon/decision-staging.md` "Rule"). */
const OPEN_ITEM: KnownProposalStatus = "open";

/** Whether an item takes an answer now: an open one alone. */
export function takesAnswer(status: string): boolean {
  return status === OPEN_ITEM;
}

/** The state whose decision note is the item's answer (docs/features/card-questions.md "Acceptance criteria", U1): a reject's reason. */
const REJECTED_ITEM: KnownProposalStatus = "rejected";

/** Whether an item's decision note is its answer: a rejected item's reason is; an approver's note on any other is not. */
export function noteAnswers(status: string): boolean {
  return status === REJECTED_ITEM;
}

/** The two kinds a card raises, as the card door takes them (docs/features/card-questions.md "Acceptance criteria", L3). */
export const RAISED_KINDS = { question: "question", decision: "decision" } as const;

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

/**
 * How each status category looks (docs/features/design-foundation.md "Status categories"): its
 * lozenge's appearance and icon; the label is the column's served title (category.ts). Stranded and
 * to do are neutral, never in progress's information blue; returned warns and done succeeds.
 */
const CATEGORY_MARKS: Readonly<Record<StatusCategory, { appearance: LozengeAppearance; icon: IconName }>> = {
  stranded: { appearance: "neutral", icon: "unknown" },
  done: { appearance: "success", icon: "taskAccepted" },
  returned: { appearance: "warning", icon: "cardReturned" },
  todo: { appearance: "neutral", icon: "open" },
  doing: { appearance: "information", icon: "taskInProgress" },
};

/** A category's appearance and icon. */
export function categoryMark(category: StatusCategory): { appearance: LozengeAppearance; icon: IconName } {
  return CATEGORY_MARKS[category];
}

/**
 * Each word of a parent's progress bar (docs/features/handoff-progress.md "Acceptance criteria",
 * HP-3): its fill, a lozenge appearance's text role, and its icon; its words the catalog's
 * `board.progress` plural. Accepted succeeds, returned warns, in progress informs; cancelled, to do
 * and not in the flow are neutral, told apart by their icons and words, never by colour alone.
 */
const PROGRESS_MARKS: Readonly<Record<ProgressWord, { appearance: LozengeAppearance; icon: IconName }>> = {
  accepted: { appearance: "success", icon: "taskAccepted" },
  cancelled: { appearance: "neutral", icon: "taskCancelled" },
  returned: { appearance: "warning", icon: "cardReturned" },
  doing: { appearance: "information", icon: "taskInProgress" },
  todo: { appearance: "neutral", icon: "open" },
  stranded: { appearance: "neutral", icon: "unknown" },
};

/** A progress word's fill and icon. */
export function progressMark(word: ProgressWord): { appearance: LozengeAppearance; icon: IconName } {
  return PROGRESS_MARKS[word];
}

/** A progress word with its count, in the viewer's plural: "3 accepted". */
export function progressWords(word: ProgressWord, count: number, t: Translate): string {
  return t(`board.progress.${word}`, { count });
}

/**
 * A parent whose every child is accepted or cancelled: a status, the owner's word (v5 Q3), never a
 * control; Accept stands where it stands (ADR-0074).
 */
export function readyLook(t: Translate): LozengeLook {
  return { appearance: "success", icon: "taskAccepted", label: t("board.progress.ready") };
}
