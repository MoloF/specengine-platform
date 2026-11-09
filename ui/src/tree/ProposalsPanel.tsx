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
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

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
  const t = useTranslate();
  const now = useNow();
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(inbox.data, () => region.current);

  if (inbox.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={t("tree.proposals.loading")} lines={3} />
      </div>
    ) : (
      <ErrorPanel
        title={t("tree.proposals.failed")}
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
      <p className="muted">{t("tree.proposals.note")}</p>
      {matching.length === 0 ? (
        <div className="empty-state">
          <p>{t("tree.proposals.none")}</p>
          <p>{fill(t("tree.proposals.next", { link: slot("link") }), { link: <a href={sectionHash(project, "inbox")}>{t("tree.proposals.nextLink")}</a> })}</p>
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
                <Badge name={t("tree.proposals.status")} look={statusLook(proposal.status, t)} />
                <Badge name={t("tree.proposals.severity")} look={severityLook(proposal.severity, t)} />
                <time className="muted" dateTime={proposal.created_at} title={formatUtc(proposal.created_at, t)}>
                  {formatAge(proposal.created_at, now, t)}
                </time>
              </p>
              <p>{summaryOf(proposal, t)}</p>
            </li>
          ))}
        </ul>
      )}
      {inbox.error !== null && (
        <p className="note" role="alert">
          {fill(t("tree.proposals.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(inbox.error).message}</span> })}
        </p>
      )}
    </div>
  );
}
