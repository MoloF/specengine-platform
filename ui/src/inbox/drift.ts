import type { DriftEvidence } from "../api/types";
import { PROPOSAL_ID } from "./decisions";

// A drift item (`docs/features/drift-queue.md` "Review, inbox, round, stage", "Approve"): `spec
// verify`'s drift of one node, queued for the owner with its evidence. It is decided on a terminal
// only, where it was raised, and never staged: the card offers its commands in place of the
// decisions. The review document's `drift` key says a proposal is one, never its kind (ADR-0031).

export type DriftAction = "approve" | "editorial" | "reject";

export interface DriftCommand {
  action: DriftAction;
  /** What it does, in words. */
  meaning: string;
  /** The command exactly as copied. */
  text: string;
}

/**
 * The terminal commands deciding a drift item: approve (the lock accept), approve `--editorial` (a
 * node's text changed at its locked revision, wording only: the terminal names it when it refuses a
 * plain approve for that), reject with a reason. Fixed words and the checked ID only; null when the
 * ID is not a proposal ID: no command is offered for it.
 */
export function driftCommands(id: string): DriftCommand[] | null {
  if (!PROPOSAL_ID.test(id)) {
    return null;
  }
  return [
    {
      action: "approve",
      meaning: "Approve: shows the rows, asks [y/N], then locks the node and its code at HEAD as spec lock accept does, in a commit of spec.lock alone",
      text: `spec approve ${id}`,
    },
    {
      action: "editorial",
      meaning:
        "Approve as editorial: only when the node's text changed at its locked revision and keeps its meaning; a plain approve refused for that names this command",
      text: `spec approve ${id} --editorial`,
    },
    {
      action: "reject",
      meaning: "Reject: spec.lock stays as it is, and the same evidence is not queued again; type your reason in place of ...",
      text: `spec reject ${id} --reason "..."`,
    },
  ];
}

/** How many rows the evidence counts, the stored ones and those past them. */
export function driftRowsText(drift: DriftEvidence): string {
  const total = drift.rows.length + drift.more;
  const rows = `${String(total)} ${total === 1 ? "row" : "rows"}`;
  return drift.more > 0 ? `${rows}, the first ${String(drift.rows.length)} stored with the item` : rows;
}

/** A row as sent: its compact JSON, untyped until Phase 4's drift screen. */
export function driftRowText(row: unknown): string {
  return JSON.stringify(row);
}
