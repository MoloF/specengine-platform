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

const VERIFY = "Verify: judges the node's own bindings against spec.lock, as this tab does; a section by its own REF";
const DRIFT = "Queue its drift: judges the node's own rows at HEAD (commit edits first) and queues an Inbox item for your decision";
const ACCEPT = "Accept: asks [y/N], then locks the node and its code at HEAD in a commit of spec.lock alone";

/**
 * The commands for the REF the tab was opened with (the route's, which the daemon resolved):
 * verify its bindings, queue its drift, accept its lock. `verifyOnly`: verify alone, for a REF
 * naming several holders or a document with no ID, since `propose drift` and `lock accept` each
 * need one node with an ID. Null when the REF is not of NODE_ID's form: no command is offered for it.
 */
export function nodeCommands(ref: string, verifyOnly = false): NodeCommand[] | null {
  if (!NODE_ID.test(ref)) {
    return null;
  }
  const verify: NodeCommand = { action: "verify", meaning: VERIFY, text: `spec verify --node ${ref}` };
  if (verifyOnly) {
    return [verify];
  }
  return [verify, { action: "drift", meaning: DRIFT, text: `spec propose drift ${ref}` }, { action: "accept", meaning: ACCEPT, text: `spec lock accept ${ref}` }];
}
