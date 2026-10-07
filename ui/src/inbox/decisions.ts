/**
 * The owner's four decisions on a proposal, as the Inbox offers them (06 §3.3): each a button, a key
 * and a dialog. Only accept and reject are staged, and confirmed on a terminal
 * (`docs/canon/decision-staging.md` "UI"); the other two are not built: nothing is sent.
 */
export type DecisionKind = "accept" | "reject" | "needs_clarification" | "defer";

interface Wording {
  /** The button, the shortcut list and the dialog's title say this. */
  verb: string;
  title: (id: string) => string;
  effect: string;
  /** The daemon stages it; false: not built, the dialog sends nothing. */
  staged: boolean;
  /** The dialog's submit: what is staged. */
  submit: string;
  field: string;
  required: boolean;
  missing: string;
}

/** The four decisions (06 §3.3): what each is called, what it does, what it asks for. */
export const WORDING: Record<DecisionKind, Wording> = {
  accept: {
    verb: "Accept",
    title: (id) => `Accept ${id}`,
    effect:
      "Stages your approval; nothing is applied here. Confirm it on a terminal: spec approve with this ID shows the staged choice and its time and asks [y/N], then applies it as its own commit.",
    staged: true,
    submit: "Stage accept",
    field: "Note for the record (optional)",
    required: false,
    missing: "",
  },
  reject: {
    verb: "Reject",
    title: (id) => `Reject ${id}`,
    effect:
      "Stages the rejection with your reason; nothing changes here. Confirm it on a terminal: spec reject with this ID shows the reason and asks [y/N]; an agent proposing the same again then gets the reason back.",
    staged: true,
    submit: "Stage reject",
    field: "Reason (required)",
    required: true,
    missing: "Write a reason: the author reads it, and so does the next agent proposing the same.",
  },
  needs_clarification: {
    verb: "Needs clarification",
    title: (id) => `Needs clarification: ${id}`,
    effect: "The author would see your question and extend the proposal; it would stay in the queue.",
    staged: false,
    submit: "",
    field: "",
    required: false,
    missing: "",
  },
  defer: {
    verb: "Defer",
    title: (id) => `Defer ${id}`,
    effect: "The proposal would stay in the queue with a mark; work goes on with the working answer.",
    staged: false,
    submit: "",
    field: "",
    required: false,
    missing: "",
  },
};

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
 * neither of the two.
 */
export function confirmCommand(decision: string, id: string): string | null {
  const words = CONFIRM_WORDS.get(decision);
  return words === undefined || !PROPOSAL_ID.test(id) ? null : `${words} ${id}`;
}
