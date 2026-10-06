import type { KnownTaskStatus, TaskStatus } from "../api/types";

// The owner's actions on a task, as commands for a terminal (docs/canon/architecture.md#control,
// ADR-0012): the browser changes nothing; the terminal asks [y/N]. Which action applies in which
// state is the transition table of `docs/features/task-package.md` "Data" (owner rows: approve,
// changes, cancel); this module is the only copy of it in the UI.

export type OwnerAction = "approve" | "changes" | "cancel";

/** Per state, the owner's actions the transition table allows there; any other state: none. */
const ACTIONS: Partial<Record<KnownTaskStatus, readonly OwnerAction[]>> = {
  draft: ["approve", "cancel"],
  changes_requested: ["approve", "cancel"],
  review: ["approve", "changes", "cancel"],
  ready: ["approve", "cancel"],
  in_progress: ["cancel"],
};

/** Only an ID of this form enters a command: fixed words and a checked ID, nothing else. */
export const TASK_ID = /^T-[0-9]{4,}$/;

/** The owner actions open in a state, in table order. */
export function actionsFor(status: TaskStatus): readonly OwnerAction[] {
  return Object.hasOwn(ACTIONS, status) ? (ACTIONS[status as KnownTaskStatus] ?? []) : [];
}

export interface OwnerCommand {
  /** The action, or `show` for the read every task gets. */
  action: OwnerAction | "show";
  /** What it does, in words. */
  meaning: string;
  /** The command exactly as copied. */
  text: string;
}

function meaningOf(action: OwnerAction, status: TaskStatus): string {
  switch (action) {
    case "approve":
      return status === "ready" ? "Approve again: re-freezes the snapshot" : "Approve: the task becomes ready";
    case "changes":
      return "Request changes: type your note in place of ...";
    case "cancel":
      return "Cancel the task";
  }
}

function textOf(action: OwnerAction, id: string): string {
  switch (action) {
    case "approve":
      return `spec task approve ${id}`;
    case "changes":
      return `spec task changes ${id} --note "..."`;
    case "cancel":
      return `spec task cancel ${id}`;
  }
}

/**
 * The commands for a task: its state's row of the table, then `spec task show`. Null when the ID
 * is not a task ID: no command is offered for it.
 */
export function ownerCommands(id: string, status: TaskStatus | null): OwnerCommand[] | null {
  if (!TASK_ID.test(id)) {
    return null;
  }
  const actions = status === null ? [] : actionsFor(status);
  return [
    ...actions.map((action) => ({ action, meaning: meaningOf(action, status ?? ""), text: textOf(action, id) })),
    { action: "show" as const, meaning: "Read the task on the terminal", text: `spec task show ${id}` },
  ];
}
