import type { Proposal } from "../api/types";
import type { Translate } from "../i18n";
import { formatUtc } from "../ui/time";

/**
 * The owner's four decisions on a proposal, as the Inbox offers them (06 §3.3): each a button, a key
 * and a dialog. Only accept and reject are staged, and confirmed on a terminal
 * (`docs/canon/decision-staging.md` "UI"); the other two are not built: nothing is sent.
 */
export type DecisionKind = "accept" | "reject" | "needs_clarification" | "defer";

interface Wording {
  /** The button, the shortcut list and the dialog's title say this. */
  verb: string;
  title: string;
  effect: string;
  /** The daemon stages it; false: not built, the dialog sends nothing. */
  staged: boolean;
  /** The dialog's submit: what is staged. */
  submit: string;
  field: string;
  required: boolean;
  missing: string;
}

/** Whether the daemon stages the decision: Accept and Reject; the other two are not built, nothing is sent. */
export function isStaged(kind: DecisionKind): boolean {
  return kind === "accept" || kind === "reject";
}

/** What a decision is called: its button, its key in the shortcut list. */
export function verbOf(kind: DecisionKind, t: Translate): string {
  switch (kind) {
    case "accept":
      return t("inbox.decision.accept.verb");
    case "reject":
      return t("inbox.decision.reject.verb");
    case "needs_clarification":
      return t("inbox.decision.needsClarification.verb");
    case "defer":
      return t("inbox.decision.defer.verb");
  }
}

/** The four decisions (06 §3.3): what each is called, what it does, what it asks for, in the language `t` reads. */
export function wordingOf(kind: DecisionKind, id: string, t: Translate): Wording {
  const verb = verbOf(kind, t);
  switch (kind) {
    case "accept":
      return {
        verb,
        title: t("inbox.decision.accept.title", { id }),
        effect: t("inbox.decision.accept.effect"),
        staged: true,
        submit: t("inbox.decision.accept.submit"),
        field: t("inbox.decision.accept.field"),
        required: false,
        missing: "",
      };
    case "reject":
      return {
        verb,
        title: t("inbox.decision.reject.title", { id }),
        effect: t("inbox.decision.reject.effect"),
        staged: true,
        submit: t("inbox.decision.reject.submit"),
        field: t("inbox.decision.reject.field"),
        required: true,
        missing: t("inbox.decision.reject.missing"),
      };
    case "needs_clarification":
      return { verb, title: t("inbox.decision.needsClarification.title", { id }), effect: t("inbox.decision.needsClarification.effect"), staged: false, submit: "", field: "", required: false, missing: "" };
    case "defer":
      return { verb, title: t("inbox.decision.defer.title", { id }), effect: t("inbox.decision.defer.effect"), staged: false, submit: "", field: "", required: false, missing: "" };
  }
}

/**
 * What a Confirm says once the daemon answered, from the review document it answered: the state as
 * sent and the commit `apply_proposal` made, when one was (`docs/features/hub-confirm.md`). The click
 * pushes nothing, on every hub; a `--push` hub's sync sends the commit later (`docs/features/hub-git.md`).
 */
export function confirmedLine(id: string, review: Proposal, t: Translate): string {
  const state = review.status ?? t("inbox.noStateSent");
  return review.applied_commit === null ? t("inbox.confirmed", { id, state }) : t("inbox.confirmedCommit", { id, state, commit: review.applied_commit });
}

/** The decisions in the order and with the keys of 06 §3.3; keys are lower case, as typed. */
export const DECISION_KEYS: readonly (readonly [DecisionKind, string])[] = [
  ["accept", "a"],
  ["reject", "r"],
  ["needs_clarification", "c"],
  ["defer", "d"],
];

/** Only an ID of this form enters a command to copy: fixed words and a checked ID, nothing else. */
export const PROPOSAL_ID = /^PR-[0-9]{4,}$/;

/** The command's fixed words per staged decision: a staged reject needs no `--reason`. */
const CONFIRM_WORDS = new Map([
  ["approve", "spec approve"],
  ["reject", "spec reject"],
]);

/**
 * The terminal command confirming a staged choice: `spec approve PR-0004` or `spec reject PR-0004`,
 * fixed words and the checked ID only; null when the ID is not a proposal ID or the decision is
 * neither of the two. An editorial approve's is bare too: the bare command confirms the stage, its
 * note kept, and typed flags would replace it whole
 * (`docs/features/rev-discipline.md` "Review document, stage, queue").
 */
export function confirmCommand(decision: string, id: string): string | null {
  const words = CONFIRM_WORDS.get(decision);
  return words === undefined || !PROPOSAL_ID.test(id) ? null : `${words} ${id}`;
}

/**
 * What a stage says once stored, the Inbox's dialog and a card's Answer… alike: the decision, when it
 * was staged as every stored time is shown, and the terminal command that confirms it
 * (`docs/canon/decision-staging.md` "UI"; docs/features/card-questions.md "Acceptance criteria", U4).
 */
export function stagedAnnouncement(id: string, decision: "approve" | "reject", review: Pick<Proposal, "staged_at">, t: Translate): string {
  const verb = verbOf(decision === "approve" ? "accept" : "reject", t);
  const command = confirmCommand(decision, id);
  const confirm = command === null ? t("inbox.confirmOnTerminal") : t("inbox.confirmCommand", { command });
  return review.staged_at === null ? t("inbox.stagedSaid", { verb, id, confirm }) : t("inbox.stagedSaidAt", { verb, id, at: formatUtc(review.staged_at, t), confirm });
}
