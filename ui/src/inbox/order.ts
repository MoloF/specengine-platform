import type { Proposal } from "../api/types";
import { severityRank } from "./labels";

const byId = new Intl.Collator("en", { numeric: true });

function time(stored: string): number {
  const at = Date.parse(stored);
  return Number.isNaN(at) ? Number.POSITIVE_INFINITY : at;
}

/** Queue order: severity high, normal, low, then none or unknown; oldest first; then ID. */
export function queueOrder(proposals: readonly Proposal[]): Proposal[] {
  return [...proposals].sort(
    (a, b) =>
      severityRank(a.severity) - severityRank(b.severity) ||
      time(a.created_at) - time(b.created_at) ||
      byId.compare(a.id, b.id),
  );
}
