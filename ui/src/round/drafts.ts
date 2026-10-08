import type { RoundAnswerBody, RoundItem, StageChoice } from "../api/types";
import { ANSWER_MAX_BYTES, REASON_MAX_BYTES } from "./text";

// The owner's answers on the Questions screen before they are staged (`docs/features/ui-round.md`
// "Description and interactions"): in this tab's memory per project, keyed by the item's ID with
// the `updated_at` of the version shown when the owner picked, so a re-read that reorders the sheet
// keeps each answer on its item, and a newer version marks it changed and leaves it out until it is
// picked again. Nothing is picked for the owner: no draft is "Leave open". A reload drops them.

/**
 * What the owner picked for an item: one of its options by index, its working answer, an answer of
 * their own, a reject with its reason, or nothing (left open). Which picks an item offers follows
 * its structure, its options and its working answer, never its kind.
 */
export type DraftPick = "open" | "option" | "working" | "answer" | "reject";

/** One item's draft: the pick, the texts typed for it (kept across picks), the version it was made on. */
export interface Draft {
  readonly pick: DraftPick;
  /** The option picked, an index into the item's `options`. */
  readonly option: number | null;
  /** The owner's own answer as typed. */
  readonly answer: string;
  /** The reject's reason as typed. */
  readonly reason: string;
  /** The item's `updated_at` when the owner last picked or typed: the version the answer is staged on. */
  readonly updatedAt: string;
}

/** The drafts of one project's sheet, by item ID. */
export type Drafts = ReadonlyMap<string, Draft>;

/** A fresh draft of an item as shown. */
export function draftOf(item: RoundItem): Draft {
  return { pick: "open", option: null, answer: "", reason: "", updatedAt: item.updated_at };
}

/**
 * Whether the item shown now offers the draft's pick: by its structure, options or none, never its
 * kind. A revised item may no longer offer it (its options gone, added or fewer): such a pick is
 * never sent, and never kept for the new version.
 */
export function offers(item: RoundItem, draft: Draft): boolean {
  switch (draft.pick) {
    case "open":
    case "reject":
      return true;
    case "option":
      return item.options.length > 0 && draft.option !== null && draft.option >= 0 && draft.option < item.options.length;
    case "answer":
      return item.options.length === 0;
    case "working":
      return item.options.length === 0 && item.working_answer !== null;
  }
}

/** Where a draft stands against the item shown now. */
export type DraftState =
  /** Left open: nothing is sent. */
  | "open"
  /** Sent with the next Stage. */
  | "ready"
  /** Made on an older version of the item (or a pick it no longer offers): left out until picked again. */
  | "changed"
  /** An own answer or a reject with no text yet: left out until written. */
  | "unwritten";

/** The text a pick sends: its own answer or its reason, as typed less surrounding white space. */
export function textOf(draft: Draft): string {
  return (draft.pick === "reject" ? draft.reason : draft.answer).trim();
}

/** The cap of the text a pick sends: an answer's 2 048 bytes, a reason's 4 096; null when the pick sends none. */
export function capOf(pick: DraftPick): number | null {
  if (pick === "answer") {
    return ANSWER_MAX_BYTES;
  }
  return pick === "reject" ? REASON_MAX_BYTES : null;
}

export function stateOf(item: RoundItem, draft: Draft | undefined): DraftState {
  if (draft === undefined || draft.pick === "open") {
    return "open";
  }
  if (!offers(item, draft)) {
    // Shown as Leave open: nothing of it is sent, whatever version it was made on.
    return draft.updatedAt === item.updated_at ? "open" : "changed";
  }
  if (draft.updatedAt !== item.updated_at) {
    return "changed";
  }
  if ((draft.pick === "answer" || draft.pick === "reject") && textOf(draft) === "") {
    return "unwritten";
  }
  return "ready";
}

/**
 * The choice a ready draft stages, as the decision route takes it: an option as `option`, the
 * working answer as an approve with every flag null, an own answer as `answer`, a reject with its
 * reason; never a note or a canon (W-9). Over a cap it is sent all the same: the daemon's refusal
 * decides, nothing is cut (W-14).
 */
export function choiceOf(draft: Draft): StageChoice | null {
  switch (draft.pick) {
    case "option":
      return draft.option === null ? null : { decision: "approve", option: draft.option, answer: null, canon: null, note: null };
    case "working":
      return { decision: "approve", option: null, answer: null, canon: null, note: null };
    case "answer":
      return { decision: "approve", option: null, answer: textOf(draft), canon: null, note: null };
    case "reject":
      return { decision: "reject", reason: textOf(draft) };
    case "open":
      return null;
  }
}

/** One item's answer for the round POST: the choice, then the `updated_at` of the version shown. */
export function answerOf(item: RoundItem, draft: Draft | undefined): RoundAnswerBody | null {
  if (draft === undefined || stateOf(item, draft) !== "ready") {
    return null;
  }
  const choice = choiceOf(draft);
  return choice === null ? null : { id: item.id, stage: { ...choice, updated_at: item.updated_at } };
}

/** The drafts with the one of `id` replaced, or removed when none is given. */
export function withDraft(drafts: Drafts, id: string, draft: Draft | undefined): Drafts {
  const next = new Map(drafts);
  if (draft === undefined) {
    next.delete(id);
  } else {
    next.set(id, draft);
  }
  return next;
}
