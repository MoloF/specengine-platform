import {
  KNOWN_CHECK_VERDICTS,
  type CheckMode,
  type CheckVerdict,
  type FindingSeverity,
  type KnownCheckMode,
  type KnownCheckVerdict,
  type KnownFindingSeverity,
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
