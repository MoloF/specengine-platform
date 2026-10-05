import type { Decision } from "../api/types";

export type DecisionKind = Decision["decision"];

interface Wording {
  /** The button, the shortcut list and the dialog's submit say this. */
  verb: string;
  title: (id: string) => string;
  effect: string;
  field: string;
  required: boolean;
  missing: string;
}

/** The four decisions (06 §3.3, §3.4): what each is called, what it does, what it asks for. */
export const WORDING: Record<DecisionKind, Wording> = {
  accept: {
    verb: "Accept",
    title: (id) => `Accept ${id}`,
    effect:
      "The daemon applies it with apply_proposal; a spec change lands in the task's worktree as its own commit, spec: apply with this ID.",
    field: "Note for the record (optional)",
    required: false,
    missing: "",
  },
  reject: {
    verb: "Reject",
    title: (id) => `Reject ${id}`,
    effect: "The proposal closes with your reason; an agent proposing the same again gets the reason back.",
    field: "Reason (required)",
    required: true,
    missing: "Write a reason: the author reads it, and so does the next agent proposing the same.",
  },
  needs_clarification: {
    verb: "Needs clarification",
    title: (id) => `Needs clarification: ${id}`,
    effect: "The author sees your question and extends the proposal; it stays in the queue.",
    field: "What should the author clarify? (required)",
    required: true,
    missing: "Write what the author should clarify.",
  },
  defer: {
    verb: "Defer",
    title: (id) => `Defer ${id}`,
    effect: "The proposal stays in the queue with a mark; work goes on with the working answer.",
    field: "Note (optional)",
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
