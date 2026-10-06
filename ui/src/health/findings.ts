import type { CheckFinding, FindingSeverity, InboxEntry, Severity } from "../api/types";
import { severityRank } from "../inbox/labels";
import { folded } from "../tasks/filter";
import { findingSeverityLook, findingSeverityRank } from "./labels";

// What Health shows of one report, arranged and never judged (ADR-0012): findings grouped by their
// raw `code`, the debt in its stored order, the budget findings; the queue's severities counted. No
// verdict, count of the report, W or headroom is derived here, and no `message` is parsed.

/** The one code the screen names: a document over its budget (`docs/canon/spec-check.md` "Findings, debt, verdict"). */
export const BUDGET_CODE = "budget";

/** Groups up to this long show open; longer ones are collapsed until opened. */
export const SHOWN_OPEN = 10;

/** A finding and its place in the report, which keys it on screen. */
export interface FindingRow {
  key: number;
  finding: CheckFinding;
}

/** The findings of one code, errors first. */
export interface FindingGroup {
  code: string;
  rows: FindingRow[];
}

const collator = new Intl.Collator("en");

export function rowsOf(findings: readonly CheckFinding[]): FindingRow[] {
  return findings.map((finding, key) => ({ key, finding }));
}

/** Errors, warnings, then unknown severities by their text; the report's order within each. */
function byRow(a: FindingRow, b: FindingRow): number {
  const [x, y] = [a.finding.severity, b.finding.severity];
  return findingSeverityRank(x) - findingSeverityRank(y) || (x === y ? 0 : collator.compare(x, y)) || a.key - b.key;
}

/** The worst severity the rows hold, as a rank: a loop, never a spread (a large report has no argument limit). */
export function worstRank(rows: readonly FindingRow[]): number {
  let worst = Number.POSITIVE_INFINITY;
  for (const row of rows) {
    worst = Math.min(worst, findingSeverityRank(row.finding.severity));
  }
  return worst;
}

function groupRank(group: FindingGroup): number {
  return worstRank(group.rows);
}

/** The rows by raw `code`: a group holding an error first, then warnings, then the rest; by code within. */
export function groupsOf(rows: readonly FindingRow[]): FindingGroup[] {
  const byCode = new Map<string, FindingRow[]>();
  for (const row of rows) {
    const members = byCode.get(row.finding.code);
    if (members === undefined) {
      byCode.set(row.finding.code, [row]);
    } else {
      members.push(row);
    }
  }
  return [...byCode]
    .map(([code, members]) => ({ code, rows: [...members].sort(byRow) }))
    .sort((a, b) => groupRank(a) - groupRank(b) || collator.compare(a.code, b.code));
}

/** The chips toggled off and the text typed: they filter the one report read, never read again. */
export interface FindingFilters {
  off: ReadonlySet<string>;
  query: string;
}

export const NO_FILTERS: FindingFilters = { off: new Set(), query: "" };

export function severityChip(value: FindingSeverity): string {
  return `severity:${value}`;
}

export function codeChip(code: string): string {
  return `code:${code}`;
}

export interface Chip {
  key: string;
  label: string;
  count: number;
}

function counted(rows: readonly FindingRow[], keyOf: (row: FindingRow) => string): Map<string, number> {
  const counts = new Map<string, number>();
  for (const row of rows) {
    const key = keyOf(row);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return counts;
}

/** A chip per severity present (errors first, unknown ones raw and last), counted over the whole report. */
export function severityChipsOf(rows: readonly FindingRow[]): Chip[] {
  return [...counted(rows, (row) => row.finding.severity)]
    .sort(([a], [b]) => findingSeverityRank(a) - findingSeverityRank(b) || collator.compare(a, b))
    .map(([severity, count]) => ({ key: severityChip(severity), label: findingSeverityLook(severity).label, count }));
}

/** A chip per code present, in the groups' order, counted over the whole report. */
export function codeChipsOf(rows: readonly FindingRow[]): Chip[] {
  const counts = counted(rows, (row) => row.finding.code);
  return groupsOf(rows).map((group) => ({ key: codeChip(group.code), label: group.code, count: counts.get(group.code) ?? 0 }));
}

/** A row with the text the filter compares, folded once per report: path, subject, message, code. */
export interface SearchableRow extends FindingRow {
  folded: readonly string[];
}

/** The rows of a report ready for the text filter (NFC, lower case, once). */
export function searchableRows(rows: readonly FindingRow[]): SearchableRow[] {
  return rows.map((row) => {
    const { path, subject, message, code } = row.finding;
    return { ...row, folded: [path, subject, message, code].map(folded) };
  });
}

/** The rows passing the chips and the text, the query folded once for all of them. */
export function visibleRows(rows: readonly SearchableRow[], filters: FindingFilters): SearchableRow[] {
  const wanted = folded(filters.query.trim());
  return rows.filter(
    (row) =>
      !filters.off.has(severityChip(row.finding.severity)) &&
      !filters.off.has(codeChip(row.finding.code)) &&
      (wanted === "" || row.folded.some((text) => text.includes(wanted))),
  );
}

export function isFiltered(filters: FindingFilters): boolean {
  return filters.off.size > 0 || filters.query.trim() !== "";
}

export function toggled(filters: FindingFilters, key: string): FindingFilters {
  const off = new Set(filters.off);
  if (!off.delete(key)) {
    off.add(key);
  }
  return { ...filters, off };
}

/** The findings in debt: expired first, then by the stored `expires` as written, then the report's order. */
export function debtRowsOf(rows: readonly FindingRow[]): FindingRow[] {
  return rows
    .filter((row) => row.finding.debt !== undefined)
    .sort((a, b) => {
      const [x, y] = [a.finding.debt, b.finding.debt];
      if (x === undefined || y === undefined) {
        return a.key - b.key;
      }
      return Number(y.expired) - Number(x.expired) || (x.expires === y.expires ? 0 : x.expires < y.expires ? -1 : 1) || a.key - b.key;
    });
}

/** Exactly the `budget` findings, in the report's order. */
export function budgetRowsOf(rows: readonly FindingRow[]): FindingRow[] {
  return rows.filter((row) => row.finding.code === BUDGET_CODE);
}

/** An `.md` path opens its node in the tree; any other path (`specengine.toml`, `""`) is text. */
export function isSpecPath(path: string): boolean {
  return path.endsWith(".md");
}

export interface SeverityCount {
  severity: Severity | null;
  count: number;
}

/** The queue's proposals per severity, in the Inbox's order of severities: high, normal, low, then none and unknown ones. */
export function severityCountsOf(proposals: readonly Pick<InboxEntry, "severity">[]): SeverityCount[] {
  const counts = new Map<Severity | null, number>();
  for (const { severity } of proposals) {
    counts.set(severity, (counts.get(severity) ?? 0) + 1);
  }
  return [...counts]
    .map(([severity, count]) => ({ severity, count }))
    .sort(
      (a, b) =>
        severityRank(a.severity) - severityRank(b.severity) ||
        (a.severity === b.severity ? 0 : a.severity === null ? -1 : b.severity === null ? 1 : collator.compare(a.severity, b.severity)),
    );
}
