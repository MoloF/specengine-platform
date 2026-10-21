import {
  KNOWN_CHECK_VERDICTS,
  type AssumedState,
  type CheckMode,
  type CheckVerdict,
  type DriftCounts,
  type FindingSeverity,
  type HealthRowState,
  type KnownCheckMode,
  type KnownCheckVerdict,
  type KnownFindingSeverity,
  type StoredDriftRow,
  type TestedState,
  type UnboundReason,
} from "../api/types";
import { syncLook } from "../tree/labels";
import type { Look } from "../ui/Badge";
import { oneLine } from "./oneLine";

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

// Row 6 of What is left, drift (docs/features/health-drift-ui.md "Data"; the body:
// docs/canon/health-drift.md "Document"): a row's class and a `[[tested]]` entry's state go through
// the two tables below, a row's state through the tree's syncLook (src/tree/labels.ts), never a
// second table of verify's states. This is the only place row 6 tells one word from another; a word
// not listed is shown raw and neutral, never fails the part.

const DRIFT_CLASS = {
  fix: { label: "Fix", tone: "finding-error", icon: "findingError" },
  check: { label: "Check", tone: "finding-warning", icon: "findingWarning" },
  cannot: { label: "Cannot verify", tone: "sync-cannot-verify", icon: "cannotCheck" },
} satisfies Record<string, Look>;

/** verify's three classes, in its summary's order (`docs/canon/spec-verify-output.md` "Text"). */
export const DRIFT_CLASSES: readonly string[] = Object.keys(DRIFT_CLASS);

/** A class row 6 does not know, or none (only an `ok` row has none, and row 6 lists none): neutral, never ok's; the word on one line (`oneLine`). */
function rawClass(label: string): Look {
  return { label: oneLine(label), tone: "sync-unknown", icon: "unknown" };
}

/** A row 6 line's class: Fix, Check, or Cannot verify in cannot-verify's role, never ok's; any other word raw, neutral. */
export function driftClassLook(value: string | null): Look {
  if (value === null) {
    return rawClass("No class");
  }
  return lookup(DRIFT_CLASS, value) ?? rawClass(value);
}

/** verify's three classes, as the tree's badges count a node's own row 6 rows (docs/features/ui-tree-drift.md). */
export type DriftClass = keyof typeof DRIFT_CLASS;

/**
 * Which of verify's three classes a row 6 row is in, for the tree's badges: the word passed in,
 * never compared elsewhere; null for a word not listed, or none (only `ok` has none, and row 6
 * lists no `ok`).
 */
export function driftClassOf(value: string | null): DriftClass | null {
  return typeof value === "string" && Object.hasOwn(DRIFT_CLASS, value) ? (value as DriftClass) : null;
}

/** A node's own row 6 rows by class, as the tree counts them; `unlisted`: any other class word, or none. */
export interface DriftClassCounts {
  fix: number;
  check: number;
  cannot: number;
  unlisted: number;
}

/** The tree's badge for rows of a class row 6 does not list (none today; the word shows raw on Health's row 6): neutral, never ok's. */
const UNLISTED_CLASS: Look = { label: "Another class", tone: "sync-unknown", icon: "unknown" };

/**
 * A tree line's drift badges (docs/features/ui-tree-drift.md "Badges"): Fix, Check, Cannot verify
 * in verify's order, then Another class, each only when its count is above 0 (never a 0), its
 * class's look with the label `<label> <count>`.
 */
export function driftBadgeLooks(counts: Readonly<DriftClassCounts>): Look[] {
  const each: [Look, number][] = [
    [DRIFT_CLASS.fix, counts.fix],
    [DRIFT_CLASS.check, counts.check],
    [DRIFT_CLASS.cannot, counts.cannot],
    [UNLISTED_CLASS, counts.unlisted],
  ];
  return each.filter(([, count]) => count > 0).map(([look, count]) => ({ ...look, label: `${look.label} ${String(count)}` }));
}

/** One count of row 6's tally: its key, its look, verify's count. */
export interface DriftTally {
  key: string;
  look: Look;
  count: number;
}

/**
 * verify's counts as row 6 draws them (`docs/canon/spec-verify-output.md` "Text": `<f> fix, <c>
 * check, <k> cannot, <o> ok`): one per class in its order, each in its class's look, then `ok` in
 * the tree's "In sync" look; read from the counts, never counted here.
 */
export function driftTally(counts: DriftCounts): DriftTally[] {
  return [
    { key: "fix", look: DRIFT_CLASS.fix, count: counts.fix },
    { key: "check", look: DRIFT_CLASS.check, count: counts.check },
    { key: "cannot", look: DRIFT_CLASS.cannot, count: counts.cannot },
    { key: "ok", look: syncLook("ok"), count: counts.ok },
  ];
}

const TESTED = {
  verified: { label: "Verified", tone: "sync-ok", icon: "checkClean" },
  stale: { label: "Stale", tone: "stale-changed", icon: "deferred" },
  unverifiable: { label: "Unverifiable", tone: "sync-cannot-verify", icon: "cannotCheck" },
  node_gone: { label: "Node gone", tone: "sync-broken-binding", icon: "linkDangling" },
} satisfies Record<string, Look>;

/** A `[[tested]]` entry's four states, in the canon's order (`docs/canon/spec-lock-tested.md` "The state"). */
export const TESTED_STATES: readonly string[] = Object.keys(TESTED);

/** A `[[tested]]` entry's state: its label, colour role and icon; unverifiable in cannot-verify's role, never verified's; any other word raw, neutral. */
export function testedLook(value: TestedState): Look {
  return lookup(TESTED, value) ?? { label: value, tone: "sync-unknown", icon: "unknown" };
}

/**
 * A drift row an editorial approve may fit: verify's `spec_ahead same_revision`, the node's text
 * changed at its locked revision (`docs/canon/spec-verify.md` "States"). The Inbox card marks such
 * a stored row "Editorial" and lists `spec approve PR --editorial` whatever its rows: they were
 * judged when the item was raised, and accept judges `HEAD` again (`docs/canon/proposal-drift.md`
 * "Approve"). The one place a row's state and category are told apart, as words passed in.
 */
export function isEditorialRow({ state, category }: Pick<StoredDriftRow, "state" | "category">): boolean {
  return state === "spec_ahead" && category === "same_revision";
}
