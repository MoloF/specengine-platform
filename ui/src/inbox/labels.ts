import type {
  KnownGapType,
  KnownPreview,
  KnownProposalStatus,
  KnownSeverity,
  GapType,
  Preview,
  ProposalStatus,
  Severity,
} from "../api/types";
import type { Look } from "../ui/Badge";

// Closed tables: their known literals get a label, a colour role and an icon; any other string
// is shown raw in a neutral badge, never mapped to a known value.

function lookup<K extends string>(table: Record<K, Look>, value: string): Look | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

const SEVERITY: Record<KnownSeverity, Look> = {
  high: { label: "High", tone: "severity-high", icon: "severityHigh" },
  normal: { label: "Normal", tone: "severity-normal", icon: "severityNormal" },
  low: { label: "Low", tone: "severity-low", icon: "severityLow" },
};

const SEVERITY_RANK: Record<KnownSeverity, number> = { high: 0, normal: 1, low: 2 };

/** After `low`: null and any unknown severity. */
export const UNRANKED = 3;

export function severityLook(value: Severity | null): Look {
  if (value === null) {
    return { label: "None", tone: "severity-unknown", icon: "unknown" };
  }
  return lookup(SEVERITY, value) ?? { label: value, tone: "severity-unknown", icon: "unknown" };
}

export function severityRank(value: Severity | null): number {
  if (value === null || !Object.hasOwn(SEVERITY_RANK, value)) {
    return UNRANKED;
  }
  return SEVERITY_RANK[value as KnownSeverity];
}

const STATUS: Record<KnownProposalStatus, Look> = {
  open: { label: "Open", tone: "proposal-open", icon: "open" },
  changes_requested: { label: "Changes requested", tone: "proposal-changes-requested", icon: "changes" },
  approved: { label: "Approved", tone: "proposal-approved", icon: "approved" },
  applied: { label: "Applied", tone: "proposal-applied", icon: "applied" },
  rejected: { label: "Rejected", tone: "proposal-rejected", icon: "rejected" },
  deferred: { label: "Deferred", tone: "proposal-deferred", icon: "deferred" },
  superseded: { label: "Superseded", tone: "proposal-superseded", icon: "superseded" },
};

export function statusLook(value: ProposalStatus): Look {
  return lookup(STATUS, value) ?? { label: value, tone: "proposal-unknown", icon: "unknown" };
}

const STATUS_ORDER: readonly string[] = Object.keys(STATUS);

/** A status's place in the table's order (open first); any unknown value after every known one. */
export function statusRank(value: ProposalStatus): number {
  const at = STATUS_ORDER.indexOf(value);
  return at === -1 ? STATUS_ORDER.length : at;
}

const GAP: Record<KnownGapType, string> = {
  missing: "Missing in the spec or the code",
  partial: "Partly covered",
  contradicts: "Contradicts the spec",
  unrequested: "Code nobody asked for",
};

/** A gap type's label; the raw text when unknown, `null` when absent. */
export function gapLabel(value: GapType | null): string | null {
  if (value === null) {
    return null;
  }
  return Object.hasOwn(GAP, value) ? GAP[value as KnownGapType] : value;
}

const PREVIEW: Record<KnownPreview, string> = {
  applies: "Applies to the current text as is",
  rebases: "Rebases onto the current text",
  conflicts: "Conflicts with the current text",
  unavailable: "Preview unavailable",
};

/** An apply preview's label; the raw text when unknown, `null` when absent. */
export function previewLabel(value: Preview | null): string | null {
  if (value === null) {
    return null;
  }
  return Object.hasOwn(PREVIEW, value) ? PREVIEW[value as KnownPreview] : value;
}
