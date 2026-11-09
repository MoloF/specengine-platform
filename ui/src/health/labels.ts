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
import type { Translate } from "../i18n";
import { syncLook } from "../tree/labels";
import type { Look } from "../ui/Badge";
import { oneLine } from "./oneLine";

// The check's closed tables (`docs/canon/spec-check.md` "Findings, debt, verdict"): a known value
// gets a label from the catalog's `health` group, a colour role and an icon; any other string is
// shown raw in a neutral badge and sorted last. A verdict is labelled through KNOWN_CHECK_VERDICTS,
// never spelled here (docs/features/ui-health.md "Rules and edge cases"); nothing here judges a report.

type Mark = Omit<Look, "label">;

const [CLEAN, OBSERVED, FAILS, CANNOT_CHECK] = KNOWN_CHECK_VERDICTS;

const VERDICT: Record<KnownCheckVerdict, Mark & { key: "clean" | "observed" | "fails" | "cannotCheck" }> = {
  [CLEAN]: { key: "clean", tone: "check-clean", icon: "checkClean" },
  [OBSERVED]: { key: "observed", tone: "check-observed", icon: "checkObserved" },
  [FAILS]: { key: "fails", tone: "check-fails", icon: "checkFails" },
  [CANNOT_CHECK]: { key: "cannotCheck", tone: "check-cannot-check", icon: "cannotCheck" },
};

function lookup<K extends string, V>(table: Record<K, V>, value: string): V | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

/** A verdict's label and look; an unknown one raw, neutral. */
export function verdictLook(value: CheckVerdict, t: Translate): Look {
  const known = lookup(VERDICT, value);
  return known === null ? { label: value, tone: "check-unknown", icon: "unknown", served: true } : { label: t(`health.verdict.${known.key}`), tone: known.tone, icon: known.icon };
}

/** The report says the corpus is clean (its own verdict, read, never derived). */
export function isClean(value: CheckVerdict): boolean {
  return value === CLEAN;
}

/** The report says the check could not vouch for the corpus: W is not measured, the causes say why. */
export function isCannotCheck(value: CheckVerdict): boolean {
  return value === CANNOT_CHECK;
}

const SEVERITY: Record<KnownFindingSeverity, Mark> = {
  error: { tone: "finding-error", icon: "findingError" },
  warning: { tone: "finding-warning", icon: "findingWarning" },
};

const SEVERITY_ORDER: readonly string[] = Object.keys(SEVERITY);

/** A finding's severity: label and icon; an unknown one raw, neutral. */
export function findingSeverityLook(value: FindingSeverity, t: Translate): Look {
  const mark = lookup(SEVERITY, value);
  return mark === null ? { label: value, tone: "finding-unknown", icon: "unknown", served: true } : { label: t(`health.severity.${value as KnownFindingSeverity}`), ...mark };
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
  return { label: value, tone: "check-unknown", icon: "unknown", served: true };
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
  measured: { measured: true, retry: false },
  off: { measured: false, retry: false },
  not_configured: { measured: false, retry: false },
  cannot: { measured: false, retry: true },
} satisfies Record<string, Omit<CodeRowLook, "label">>;

/** The four code row states, in the canon's order. */
export const CODE_ROW_STATES = Object.keys(CODE_ROW) as readonly (keyof typeof CODE_ROW)[];

/** Whether a code row's state says measured: the one state drawn with counts and rows. */
export function isMeasuredState(value: HealthRowState): boolean {
  return Object.hasOwn(CODE_ROW, value) && CODE_ROW[value as keyof typeof CODE_ROW].measured;
}

/** A code row's state: measured, not measured (off, not configured) or could not be measured; any other word raw, not measured. */
export function codeRowLook(value: HealthRowState, t: Translate): CodeRowLook {
  if (!Object.hasOwn(CODE_ROW, value)) {
    return { label: value, measured: false, retry: false };
  }
  const state = value as keyof typeof CODE_ROW;
  return { label: t(`health.codeRow.${state}`), ...CODE_ROW[state] };
}

/**
 * The words of a row whose state says measured but whose answer holds no counts or no rows (outside
 * the contract, `docs/canon/health.md` "UI"): drawn as not measured, after "Not measured: ", with
 * no digit and no Retry, never as "Measured".
 */
export function measuredWithoutCounts(t: Translate): string {
  return t("health.measuredWithoutCounts");
}

/** How a row 4 line is drawn: its state's badge and which part of the CLI's tail it carries. */
export interface AssumedLook {
  look: Look;
  /** The decision's choice and record follow the item (`as_assumed`, `otherwise`), as `spec health` prints them. */
  record: boolean;
  /** The node's open items follow it (`in_question`), instead of its status and successor. */
  under: boolean;
}

const ASSUMED = {
  pending: { mark: { tone: "assumed-pending", icon: "open" }, record: false, under: false },
  as_assumed: { mark: { tone: "assumed-as-assumed", icon: "approved" }, record: true, under: false },
  otherwise: { mark: { tone: "assumed-otherwise", icon: "changes" }, record: true, under: false },
  applied: { mark: { tone: "assumed-applied", icon: "applied" }, record: false, under: false },
  rejected: { mark: { tone: "assumed-rejected", icon: "rejected" }, record: false, under: false },
  unknown: { mark: { tone: "assumed-missing", icon: "linkDangling" }, record: false, under: false },
  archived: { mark: { tone: "assumed-archived", icon: "archived" }, record: false, under: false },
  in_question: { mark: { tone: "assumed-in-question", icon: "waiting" }, record: false, under: true },
} satisfies Record<string, { mark: Mark; record: boolean; under: boolean }>;

/** Row 4's eight states (a marker on a queue item, an `@assumes` on an archived node or one in question), in the canon's order. */
export const ASSUMED_STATES: readonly string[] = Object.keys(ASSUMED);

/** A row 4 line's state: label, colour role, icon, the tail it carries; any other word raw, neutral, no record, no list. */
export function assumedLook(value: AssumedState, t: Translate): AssumedLook {
  if (!Object.hasOwn(ASSUMED, value)) {
    return { look: { label: value, tone: "assumed-unknown", icon: "unknown", served: true }, record: false, under: false };
  }
  const state = value as keyof typeof ASSUMED;
  const { mark, record, under } = ASSUMED[state];
  return { look: { label: t(`health.assumed.${state}`), ...mark }, record, under };
}

const UNBOUND_REASON = {
  homoglyph: { tone: "unbound-homoglyph", icon: "findingWarning" },
  unanchored: { tone: "unbound-unanchored", icon: "linkSkipped" },
  cannot_verify: { tone: "unbound-cannot-verify", icon: "cannotCheck" },
  relation: { tone: "unbound-relation", icon: "superseded" },
} satisfies Record<string, Mark>;

/** Row 5's four reasons a marker naming a node binds it not, in the canon's order. */
export const UNBOUND_REASONS: readonly string[] = Object.keys(UNBOUND_REASON);

/** Why a marker naming an unbound node binds it not; cannot_verify in cannot-verify's role, never ok's; any other word raw, neutral. */
export function unboundReasonLook(value: UnboundReason, t: Translate): Look {
  const mark = lookup(UNBOUND_REASON, value);
  return mark === null ? { label: value, tone: "unbound-unknown", icon: "unknown", served: true } : { label: t(`health.unbound.${value as keyof typeof UNBOUND_REASON}`), ...mark };
}

/** A marker's ID states as `spec refs` words them (docs/canon/code-bindings.md "ID state"). */
const ID_STATE = { resolved: true, alias: true, ambiguous: true, homoglyph: true, other_project: true, queue: true, dangling: true };

/** The seven ID states, in the table's order. */
export const ID_STATES = Object.keys(ID_STATE) as readonly (keyof typeof ID_STATE)[];

/** A marker's binding words as `spec refs` gives them (docs/canon/spec-cli-refs.md "Rows"). */
const BINDING = { item: true, module: true, ron: true, file: true, unanchored: true, cannot_verify: true };

/** The six binding words, in the table's order. */
export const BINDINGS = Object.keys(BINDING) as readonly (keyof typeof BINDING)[];

/** The ID state's words; any other raw. */
export function idStateLabel(value: string, t: Translate): string {
  return Object.hasOwn(ID_STATE, value) ? t(`health.idState.${value as keyof typeof ID_STATE}`) : value;
}

/** The binding's words; any other raw. */
export function bindingLabel(value: string, t: Translate): string {
  return Object.hasOwn(BINDING, value) ? t(`health.binding.${value as keyof typeof BINDING}`) : value;
}

// Row 6 of What is left, drift (docs/features/health-drift-ui.md "Data"; the body:
// docs/canon/health-drift.md "Document"): a row's class and a `[[tested]]` entry's state go through
// the two tables below, a row's state through the tree's syncLook (src/tree/labels.ts), never a
// second table of verify's states. This is the only place row 6 tells one word from another; a word
// not listed is shown raw and neutral, never fails the part.

const DRIFT_CLASS = {
  fix: { tone: "finding-error", icon: "findingError" },
  check: { tone: "finding-warning", icon: "findingWarning" },
  cannot: { tone: "sync-cannot-verify", icon: "cannotCheck" },
} satisfies Record<string, Mark>;

/** A class's look: its catalog label, its class's colour role and icon. */
function classLook(value: keyof typeof DRIFT_CLASS, t: Translate): Look {
  return { label: t(`health.driftClass.${value}`), ...DRIFT_CLASS[value] };
}

/** verify's three classes, in its summary's order (`docs/canon/spec-verify-output.md` "Text"). */
export const DRIFT_CLASSES: readonly string[] = Object.keys(DRIFT_CLASS);

/** A class row 6 does not know, or none (only an `ok` row has none, and row 6 lists none): neutral, never ok's; the word on one line (`oneLine`). */
function rawClass(label: string, served: boolean): Look {
  const look: Look = { label: oneLine(label), tone: "sync-unknown", icon: "unknown" };
  return served ? { ...look, served: true } : look;
}

/** A row 6 line's class: Fix, Check, or Cannot verify in cannot-verify's role, never ok's; any other word raw, neutral. */
export function driftClassLook(value: string | null, t: Translate): Look {
  if (value === null) {
    return rawClass(t("health.noClass"), false);
  }
  return Object.hasOwn(DRIFT_CLASS, value) ? classLook(value as keyof typeof DRIFT_CLASS, t) : rawClass(value, true);
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
function unlistedClass(t: Translate): Look {
  return { label: t("health.anotherClass"), tone: "sync-unknown", icon: "unknown" };
}

/**
 * A tree line's drift badges (docs/features/ui-tree-drift.md "Badges"): Fix, Check, Cannot verify
 * in verify's order, then Another class, each only when its count is above 0 (never a 0), its
 * class's look with the label `<label> <count>`.
 */
export function driftBadgeLooks(counts: Readonly<DriftClassCounts>, t: Translate): Look[] {
  const each: [Look, number][] = [
    [classLook("fix", t), counts.fix],
    [classLook("check", t), counts.check],
    [classLook("cannot", t), counts.cannot],
    [unlistedClass(t), counts.unlisted],
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
export function driftTally(counts: DriftCounts, t: Translate): DriftTally[] {
  return [
    { key: "fix", look: classLook("fix", t), count: counts.fix },
    { key: "check", look: classLook("check", t), count: counts.check },
    { key: "cannot", look: classLook("cannot", t), count: counts.cannot },
    { key: "ok", look: syncLook("ok", t), count: counts.ok },
  ];
}

const TESTED = {
  verified: { tone: "sync-ok", icon: "checkClean" },
  stale: { tone: "stale-changed", icon: "deferred" },
  unverifiable: { tone: "sync-cannot-verify", icon: "cannotCheck" },
  node_gone: { tone: "sync-broken-binding", icon: "linkDangling" },
} satisfies Record<string, Mark>;

/** A `[[tested]]` entry's four states, in the canon's order (`docs/canon/spec-lock-tested.md` "The state"). */
export const TESTED_STATES: readonly string[] = Object.keys(TESTED);

/** A `[[tested]]` entry's state: its label, colour role and icon; unverifiable in cannot-verify's role, never verified's; any other word raw, neutral. */
export function testedLook(value: TestedState, t: Translate): Look {
  const mark = lookup(TESTED, value);
  return mark === null ? { label: value, tone: "sync-unknown", icon: "unknown", served: true } : { label: t(`health.tested.${value as keyof typeof TESTED}`), ...mark };
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
