import type { Translate } from "../i18n";

// A node's commands for a terminal (docs/canon/architecture.md#control, ADR-0012): the browser
// runs nothing and changes nothing; each command is copied for a terminal. Fixed words and a
// checked REF only, never quoted: a REF that is not of the form below gets no command at all.

/**
 * Only a REF of this form enters a command: Latin letters and digits, then those and `.`, `_`,
 * `/`, `-` (an ID, `slug/ID`, a root-relative path). A space, a quote, `;`, `$`, a backtick, `#`, a
 * newline, a leading `-` or `.` and any non-Latin letter (a look-alike) are refused.
 */
export const NODE_ID = /^[A-Za-z0-9][A-Za-z0-9._/-]*$/;

export type NodeAction = "verify" | "drift" | "accept";

export interface NodeCommand {
  action: NodeAction;
  /** What it does, in words. */
  meaning: string;
  /** The command exactly as copied. */
  text: string;
}

/**
 * The commands for the REF the tab was opened with (the route's, which the daemon resolved):
 * verify its bindings, queue its drift, accept its lock. `verifyOnly`: verify alone, for a REF
 * naming several holders or a document with no ID, since `propose drift` and `lock accept` each
 * need one node with an ID. Null when the REF is not of NODE_ID's form: no command is offered for it.
 */
export function nodeCommands(ref: string, t: Translate, verifyOnly = false): NodeCommand[] | null {
  if (!NODE_ID.test(ref)) {
    return null;
  }
  const verify: NodeCommand = { action: "verify", meaning: t("tree.command.verify"), text: `spec verify --node ${ref}` };
  if (verifyOnly) {
    return [verify];
  }
  return [
    verify,
    { action: "drift", meaning: t("tree.command.drift"), text: `spec propose drift ${ref}` },
    { action: "accept", meaning: t("tree.command.accept"), text: `spec lock accept ${ref}` },
  ];
}
