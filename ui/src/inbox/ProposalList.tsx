import type { InboxEntry } from "../api/types";
import { Badge } from "../ui/Badge";
import { formatAge, formatUtc } from "../ui/time";
import { severityLook, statusLook } from "./labels";
import { summaryOf } from "./summary";
import { targetsOf } from "./targets";

/**
 * The queue as one Tab stop: a listbox whose selected option alone is tabbable. Moving with the
 * keys belongs to the queue around it.
 */
export function ProposalList({
  proposals,
  selectedId,
  now,
  onSelect,
}: {
  proposals: readonly InboxEntry[];
  selectedId: string;
  now: number;
  onSelect: (id: string) => void;
}) {
  return (
    <div role="listbox" aria-label="Proposals by severity, then age" className="queue-list">
      {proposals.map((proposal) => {
        const selected = proposal.id === selectedId;
        const status = statusLook(proposal.status);
        const targets = targetsOf(proposal);
        return (
          <div
            key={proposal.id}
            role="option"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            data-proposal={proposal.id}
            className="queue-item"
            onClick={() => {
              onSelect(proposal.id);
            }}
          >
            <span className="queue-item-top">
              <Badge name="Severity" look={severityLook(proposal.severity)} />
              <span className="mono queue-item-id">{proposal.id}</span>
              <time className="queue-item-age" dateTime={proposal.created_at} title={formatUtc(proposal.created_at)}>
                {formatAge(proposal.created_at, now)}
              </time>
            </span>
            <span className="queue-item-summary">{summaryOf(proposal)}</span>
            <span className="queue-item-meta">
              <span className="mono">{proposal.kind}</span>
              <span aria-hidden="true"> | </span>
              <span className="mono">{targets.length === 0 ? "no target" : targets.join(", ")}</span>
              {proposal.status !== "open" && (
                <>
                  <span aria-hidden="true"> | </span>
                  <Badge name="Status" look={status} />
                </>
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}
