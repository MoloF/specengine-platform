import type {
  Author,
  KnownGapType,
  KnownPreview,
  KnownProposalStatus,
  KnownSeverity,
  GapType,
  Preview,
  ProposalStatus,
  Severity,
} from "../api/types";
import type { Translate } from "../i18n";
import type { Look } from "../ui/Badge";
import type { IconName } from "../ui/Icon";
import type { LozengeAppearance, LozengeLook } from "../ui/Lozenge";

// Closed tables: their known literals get a label from the catalog's `inbox` group, a colour role
// and an icon; any other string is shown raw in a neutral badge, never mapped to a known value.

type Mark = Omit<Look, "label">;

function lookup<K extends string>(table: Record<K, Mark>, value: string): Mark | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

const SEVERITY: Record<KnownSeverity, Mark> = {
  high: { tone: "severity-high", icon: "severityHigh" },
  normal: { tone: "severity-normal", icon: "severityNormal" },
  low: { tone: "severity-low", icon: "severityLow" },
};

const SEVERITY_RANK: Record<KnownSeverity, number> = { high: 0, normal: 1, low: 2 };

/** After `low`: null and any unknown severity. */
export const UNRANKED = 3;

/** The three severities, in the table's order. */
export const KNOWN_SEVERITIES = Object.keys(SEVERITY) as readonly KnownSeverity[];

export function severityLook(value: Severity | null, t: Translate): Look {
  if (value === null) {
    return { label: t("inbox.severity.none"), tone: "severity-unknown", icon: "unknown" };
  }
  const mark = lookup(SEVERITY, value);
  return mark === null ? { label: value, tone: "severity-unknown", icon: "unknown", served: true } : { label: t(`inbox.severity.${value as KnownSeverity}`), ...mark };
}

export function severityRank(value: Severity | null): number {
  if (value === null || !Object.hasOwn(SEVERITY_RANK, value)) {
    return UNRANKED;
  }
  return SEVERITY_RANK[value as KnownSeverity];
}

const STATUS: Record<KnownProposalStatus, Mark> = {
  open: { tone: "proposal-open", icon: "open" },
  changes_requested: { tone: "proposal-changes-requested", icon: "changes" },
  approved: { tone: "proposal-approved", icon: "approved" },
  applied: { tone: "proposal-applied", icon: "applied" },
  rejected: { tone: "proposal-rejected", icon: "rejected" },
  deferred: { tone: "proposal-deferred", icon: "deferred" },
  superseded: { tone: "proposal-superseded", icon: "superseded" },
};

export function statusLook(value: ProposalStatus, t: Translate): Look {
  const mark = lookup(STATUS, value);
  return mark === null ? { label: value, tone: "proposal-unknown", icon: "unknown", served: true } : { label: t(`inbox.status.${value as KnownProposalStatus}`), ...mark };
}

/**
 * A proposal's status as a lozenge, the one table the Inbox's head and a task's items share
 * (docs/features/design-sweep.md "Acceptance criteria", DS5): each of the seven with its icon; a word
 * this build does not know raw and neutral, under `lang="en"`, never mapped to a known one.
 */
const STATUS_LOZENGES: Readonly<Record<KnownProposalStatus, { appearance: LozengeAppearance; icon: IconName }>> = {
  open: { appearance: "neutral", icon: "open" },
  changes_requested: { appearance: "warning", icon: "changes" },
  approved: { appearance: "information", icon: "approved" },
  applied: { appearance: "success", icon: "applied" },
  rejected: { appearance: "danger", icon: "rejected" },
  deferred: { appearance: "neutral", icon: "deferred" },
  superseded: { appearance: "neutral", icon: "superseded" },
};

export function statusLozenge(value: ProposalStatus, t: Translate): LozengeLook & { lang?: "en" } {
  if (!Object.hasOwn(STATUS_LOZENGES, value)) {
    return { appearance: "neutral", icon: "unknown", label: value, lang: "en" };
  }
  const known = value as KnownProposalStatus;
  return { ...STATUS_LOZENGES[known], label: t(`inbox.status.${known}`) };
}

const STATUS_ORDER: readonly string[] = Object.keys(STATUS);

/** The seven states, in the table's order (open first). */
export const KNOWN_PROPOSAL_STATUSES = STATUS_ORDER as readonly KnownProposalStatus[];

/** A status's place in the table's order (open first); any unknown value after every known one. */
export function statusRank(value: ProposalStatus): number {
  const at = STATUS_ORDER.indexOf(value);
  return at === -1 ? STATUS_ORDER.length : at;
}

const GAP: Record<KnownGapType, true> = { missing: true, partial: true, contradicts: true, unrequested: true };

/** The four gap types, in the table's order. */
export const KNOWN_GAP_TYPES = Object.keys(GAP) as readonly KnownGapType[];

/** A gap type's label; the raw text when unknown, `null` when absent. */
export function gapLabel(value: GapType | null, t: Translate): string | null {
  if (value === null) {
    return null;
  }
  return Object.hasOwn(GAP, value) ? t(`inbox.gap.${value as KnownGapType}`) : value;
}

const PREVIEW: Record<KnownPreview, true> = { applies: true, rebases: true, conflicts: true, unavailable: true };

/** The four previews, in the table's order. */
export const KNOWN_PREVIEWS = Object.keys(PREVIEW) as readonly KnownPreview[];

/** An apply preview's label; the raw text when unknown, `null` when absent. */
export function previewLabel(value: Preview | null, t: Translate): string | null {
  if (value === null) {
    return null;
  }
  return Object.hasOwn(PREVIEW, value) ? t(`inbox.preview.${value as KnownPreview}`) : value;
}

/** The author type whose recommendation is the AI's: an agent's (`docs/canon/proposal-queue.md` "Store"). */
const AGENT: Author["type"] = "agent";

/**
 * Who made an item's recommendation, in words (docs/features/plain-questions.md "Acceptance criteria",
 * PQ12): an agent's is the AI's; any other author's, a person's or one this build does not know, and an
 * item with none, the asker's, with no login looked up. The one place an author's type is compared.
 */
export function recommendedBy(author: Pick<Author, "type"> | null, t: Translate): string {
  return author !== null && author.type === AGENT ? t("inbox.lead.recommendedByAgent") : t("inbox.lead.recommendedByAsker");
}
