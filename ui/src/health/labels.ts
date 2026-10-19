import {
  KNOWN_CHECK_VERDICTS,
  type AssumedState,
  type CheckMode,
  type CheckVerdict,
  type FindingSeverity,
  type HealthRowState,
  type KnownCheckMode,
  type KnownCheckVerdict,
  type KnownFindingSeverity,
  type UnboundReason,
} from "../api/types";
import type { Look } from "../ui/Badge";

// The check's closed tables (`docs/canon/spec-check.md` "Findings, debt, verdict"): a known value
// gets a label, a colour role and an icon; any other string is shown raw in a neutral badge and
// sorted last. A verdict is labelled through KNOWN_CHECK_VERDICTS, never spelled here
// (docs/features/ui-health.md "Rules and edge cases"); nothing here judges a report.

const [CLEAN, OBSERVED, FAILS, CANNOT_CHECK] = KNOWN_CHECK_VERDICTS;

const VERDICT: Record<KnownCheckVerdict, Look> = {
  [CLEAN]: { label: "Clean", tone: "check-clean", icon: "checkClean" },
  [OBSERVED]: { label: "Passes with findings", tone: "check-observed", icon: "checkObserved" },
  [FAILS]: { label: "Fails the check", tone: "check-fails", icon: "checkFails" },
  [CANNOT_CHECK]: { label: "Could not check", tone: "check-cannot-check", icon: "cannotCheck" },
};

function lookup<K extends string>(table: Record<K, Look>, value: string): Look | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

/** A verdict's label and look; an unknown one raw, neutral. */
export function verdictLook(value: CheckVerdict): Look {
  return lookup(VERDICT, value) ?? { label: value, tone: "check-unknown", icon: "unknown" };
}

/** The report says the corpus is clean (its own verdict, read, never derived). */
export function isClean(value: CheckVerdict): boolean {
  return value === CLEAN;
}

/** The report says the check could not vouch for the corpus: W is not measured, the causes say why. */
export function isCannotCheck(value: CheckVerdict): boolean {
  return value === CANNOT_CHECK;
}

const SEVERITY: Record<KnownFindingSeverity, Look> = {
  error: { label: "Error", tone: "finding-error", icon: "findingError" },
  warning: { label: "Warning", tone: "finding-warning", icon: "findingWarning" },
};

const SEVERITY_ORDER: readonly string[] = Object.keys(SEVERITY);

/** A finding's severity: label and icon; an unknown one raw, neutral. */
export function findingSeverityLook(value: FindingSeverity): Look {
  return lookup(SEVERITY, value) ?? { label: value, tone: "finding-unknown", icon: "unknown" };
}

/** Errors first, then warnings, then any unknown severity. */
export function findingSeverityRank(value: FindingSeverity): number {
  const at = SEVERITY_ORDER.indexOf(value);
  return at === -1 ? SEVERITY_ORDER.length : at;
}

const MODES: Record<KnownCheckMode, true> = { observe: true, "enforce-introduced": true, enforce: true };

/** A mode the canon lists (`docs/canon/spec-check.md` "Configuration"): shown as is; another one in a neutral badge. */
export function isKnownMode(value: CheckMode): boolean {
  return Object.hasOwn(MODES, value);
}

/** An unknown mode's badge: the raw text, neutral. */
export function unknownModeLook(value: CheckMode): Look {
  return { label: value, tone: "check-unknown", icon: "unknown" };
}

// Rows 4-5 of What is left (docs/canon/health-code.md "Document", "Row 4: assumes",
// "Row 5: unbound"): every state word the two parts show goes through one of the tables below,
// the only place Health tells one word from another
// (docs/features/health-code-ui.md "Rules and edge cases"); a word not listed is shown raw, never
// fails the part, and is drawn as nothing measured.

/** How a code row is drawn in its state: its words, whether it is measured, whether a Retry may change it. */
export interface CodeRowLook {
  label: string;
  /** Counts and rows drawn, a digit in the value; else no digit, the causes. */
  measured: boolean;
  /** Could not be measured: the causes and a Retry reading the health read again. Not measured (off, not configured, a word not listed): no Retry. */
  retry: boolean;
}

const CODE_ROW = {
  measured: { label: "Measured", measured: true, retry: false },
  off: { label: "Code layer off", measured: false, retry: false },
  not_configured: { label: "Not configured", measured: false, retry: false },
  cannot: { label: "Could not be measured", measured: false, retry: true },
} satisfies Record<string, CodeRowLook>;

/** A code row's state: measured, not measured (off, not configured) or could not be measured; any other word raw, not measured. */
export function codeRowLook(value: HealthRowState): CodeRowLook {
  return Object.hasOwn(CODE_ROW, value) ? CODE_ROW[value as keyof typeof CODE_ROW] : { label: value, measured: false, retry: false };
}

/**
 * The words of a row whose state says measured but whose answer holds no counts or no rows (outside
 * the contract, `docs/canon/health.md` "UI"): drawn as not measured, after "Not measured: ", with
 * no digit and no Retry, never as "Measured".
 */
export const MEASURED_WITHOUT_COUNTS = "the answer holds no counts";

/** How a row 4 line is drawn: its state's badge and which part of the CLI's tail it carries. */
export interface AssumedLook {
  look: Look;
  /** The decision's choice and record follow the item (`as_assumed`, `otherwise`), as `spec health` prints them. */
  record: boolean;
  /** The node's open items follow it (`in_question`), instead of its status and successor. */
  under: boolean;
}

const ASSUMED = {
  pending: { look: { label: "Pending", tone: "assumed-pending", icon: "open" }, record: false, under: false },
  as_assumed: { look: { label: "Decided as assumed", tone: "assumed-as-assumed", icon: "approved" }, record: true, under: false },
  otherwise: { look: { label: "Decided otherwise", tone: "assumed-otherwise", icon: "changes" }, record: true, under: false },
  applied: { look: { label: "Applied", tone: "assumed-applied", icon: "applied" }, record: false, under: false },
  rejected: { look: { label: "Rejected", tone: "assumed-rejected", icon: "rejected" }, record: false, under: false },
  unknown: { look: { label: "No such item", tone: "assumed-missing", icon: "linkDangling" }, record: false, under: false },
  archived: { look: { label: "Archived", tone: "assumed-archived", icon: "archived" }, record: false, under: false },
  in_question: { look: { label: "In question", tone: "assumed-in-question", icon: "waiting" }, record: false, under: true },
} satisfies Record<string, AssumedLook>;

/** Row 4's eight states (a marker on a queue item, an `@assumes` on an archived node or one in question), in the canon's order. */
export const ASSUMED_STATES: readonly string[] = Object.keys(ASSUMED);

/** A row 4 line's state: label, colour role, icon, the tail it carries; any other word raw, neutral, no record, no list. */
export function assumedLook(value: AssumedState): AssumedLook {
  return Object.hasOwn(ASSUMED, value)
    ? ASSUMED[value as keyof typeof ASSUMED]
    : { look: { label: value, tone: "assumed-unknown", icon: "unknown" }, record: false, under: false };
}

const UNBOUND_REASON = {
  homoglyph: { label: "Look-alike ID", tone: "unbound-homoglyph", icon: "findingWarning" },
  unanchored: { label: "Unanchored", tone: "unbound-unanchored", icon: "linkSkipped" },
  cannot_verify: { label: "Cannot verify", tone: "unbound-cannot-verify", icon: "cannotCheck" },
  relation: { label: "Another relation", tone: "unbound-relation", icon: "superseded" },
} satisfies Record<string, Look>;

/** Row 5's four reasons a marker naming a node binds it not, in the canon's order. */
export const UNBOUND_REASONS: readonly string[] = Object.keys(UNBOUND_REASON);

/** Why a marker naming an unbound node binds it not; cannot_verify in cannot-verify's role, never ok's; any other word raw, neutral. */
export function unboundReasonLook(value: UnboundReason): Look {
  return lookup(UNBOUND_REASON, value) ?? { label: value, tone: "unbound-unknown", icon: "unknown" };
}

/** A marker's ID state as `spec refs` words it (docs/canon/code-bindings.md "ID state"). */
const ID_STATE: Record<string, string> = {
  resolved: "Resolved",
  alias: "Alias",
  ambiguous: "Ambiguous",
  homoglyph: "Look-alike",
  other_project: "Other project",
  queue: "Queue item",
  dangling: "Dangling",
};

/** A marker's binding word as `spec refs` gives it (docs/canon/spec-cli-refs.md "Rows"). */
const BINDING: Record<string, string> = {
  item: "Item",
  module: "Module",
  ron: "RON field",
  file: "File",
  unanchored: "Unanchored",
  cannot_verify: "Cannot verify",
};

/** The ID state's words; any other raw. */
export function idStateLabel(value: string): string {
  return Object.hasOwn(ID_STATE, value) ? (ID_STATE[value] ?? value) : value;
}

/** The binding's words; any other raw. */
export function bindingLabel(value: string): string {
  return Object.hasOwn(BINDING, value) ? (BINDING[value] ?? value) : value;
}
