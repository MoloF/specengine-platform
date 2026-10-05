import type { Proposal } from "../api/types";

/** Text compared by the filter: NFC, lower case, so composed and decomposed forms match. */
export function fold(text: string): string {
  return text.normalize("NFC").toLowerCase();
}

/** Whether a proposal's ID, summary, rationale, kind, task, targets, severity or status hold the query. */
export function matchesFilter(proposal: Proposal, query: string): boolean {
  const needle = fold(query.trim());
  if (needle === "") {
    return true;
  }
  const haystack = [
    proposal.id,
    proposal.summary,
    proposal.rationale,
    proposal.kind,
    proposal.task_id,
    proposal.severity,
    proposal.status,
    ...proposal.target_ids,
  ]
    .filter((part): part is string => part !== null)
    .join("\n");
  return fold(haystack).includes(needle);
}
