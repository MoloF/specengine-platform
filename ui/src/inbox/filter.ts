import type { InboxEntry } from "../api/types";

/** Text compared by the filter: NFC, lower case, so composed and decomposed forms match. */
export function fold(text: string): string {
  return text.normalize("NFC").toLowerCase();
}

/** Whether an inbox entry's ID, summary, rationale, kind, branch, targets, severity, status or record hold the query. */
export function matchesFilter(proposal: InboxEntry, query: string): boolean {
  const needle = fold(query.trim());
  if (needle === "") {
    return true;
  }
  const haystack = [
    proposal.id,
    proposal.summary,
    proposal.rationale,
    proposal.kind,
    proposal.branch,
    proposal.severity,
    proposal.status,
    proposal.record_id,
    ...proposal.target_ids,
  ]
    .filter((part): part is string => part !== null)
    .join("\n");
  return fold(haystack).includes(needle);
}
