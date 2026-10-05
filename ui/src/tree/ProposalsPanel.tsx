import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import type { InboxQuery } from "../api/queries";
import type { ShownNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { severityLook, statusLook } from "../inbox/labels";
import { summaryOf } from "../inbox/summary";
import { Badge } from "../ui/Badge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { formatAge, formatUtc } from "../ui/time";
import { useNow } from "../ui/useNow";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { proposalsFor } from "./matching";

/**
 * The Proposals tab: the inbox items targeting this node, by ID, `slug/ID` or, ID-less, path, with
 * a link to each in the Inbox. Information only: an open proposal holds nothing (ADR-0012). Reads
 * the Inbox's own query; its failure stays in this tab.
 */
export function ProposalsPanel({
  project,
  holders,
  inbox,
}: {
  project: string;
  holders: readonly ShownNode[];
  inbox: InboxQuery;
}) {
  const now = useNow();
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(inbox.data, () => region.current);

  if (inbox.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label="Loading the inbox" lines={3} />
      </div>
    ) : (
      <ErrorPanel
        title="The inbox could not be read"
        message={apiErrorOf(failure).message}
        retrying={inbox.isFetching}
        attempt={inbox.errorUpdateCount}
        onRetry={() => {
          retried();
          void inbox.refetch({ cancelRefetch: false });
        }}
      />
    );
  }
  const matching = proposalsFor(inbox.data.proposals, holders);
  return (
    <div ref={region} tabIndex={-1} className="proposals-panel">
      <p className="muted">Information only: nothing waits on these. Agents keep working with their working answer.</p>
      {matching.length === 0 ? (
        <div className="empty-state">
          <p>No inbox item targets this node.</p>
          <p>
            Next: <a href={sectionHash(project, "inbox")}>the Inbox lists every proposal waiting for you</a>.
          </p>
        </div>
      ) : (
        <ul className="node-proposals">
          {matching.map((proposal) => (
            <li key={proposal.id} className="node-proposal">
              <p className="node-proposal-head">
                <a className="mono" href={sectionHash(project, "inbox", proposal.id)}>
                  {proposal.id}
                </a>
                <span className="kind-tag">{proposal.kind}</span>
                <Badge name="Status" look={statusLook(proposal.status)} />
                <Badge name="Severity" look={severityLook(proposal.severity)} />
                <time className="muted" dateTime={proposal.created_at} title={formatUtc(proposal.created_at)}>
                  {formatAge(proposal.created_at, now)}
                </time>
              </p>
              <p>{summaryOf(proposal)}</p>
            </li>
          ))}
        </ul>
      )}
      {inbox.error !== null && (
        <p className="note" role="alert">
          The inbox could not be read again: {apiErrorOf(inbox.error).message}
        </p>
      )}
    </div>
  );
}
