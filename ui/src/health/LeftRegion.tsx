import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useInbox, type HealthQuery } from "../api/queries";
import type { Inbox } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { severityLook } from "../inbox/labels";
import { queueOrder } from "../inbox/order";
import { summaryOf } from "../inbox/summary";
import { Notes, Stored } from "../overview/parts";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { UnappliedAmendments } from "./AmendmentsPart";
import { AssumesPart } from "./AssumesPart";
import { DriftPart } from "./DriftPart";
import { severityCountsOf } from "./findings";
import { HealthRegion } from "./parts";
import { UnboundPart } from "./UnboundPart";
import { useTranslate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";

/** How many proposals of the queue are listed, in the Inbox's order. */
export const LEFT_ROWS = 5;

/** Rows 1 and 2: the queue by severity, then its first five as the Inbox lists them. */
function Queue({ project, answer }: { project: string; answer: Inbox }) {
  const t = useTranslate();
  const ordered = queueOrder(answer.proposals);
  const notes = <Notes notes={answer.notes} label={t("health.left.queueNotes")} />;
  if (ordered.length === 0) {
    return (
      <>
        {notes}
        <p className="health-left-total">{t("health.left.queueClear")}</p>
      </>
    );
  }
  const [before, after] = around(t("health.left.queueTotal", { count: ordered.length }), String(ordered.length));
  return (
    <>
      {notes}
      <p className="health-left-total">
        {before}
        <strong>{ordered.length}</strong>
        {after}
      </p>
      <ul className="health-tally" aria-label={t("health.left.bySeverity")}>
        {severityCountsOf(ordered).map(({ severity, count }) => (
          <li key={severity ?? ""}>
            <Badge look={severityLook(severity, t)} />
            <span className="sr-only">:</span> <span className="health-tally-count">{count}</span>
          </li>
        ))}
      </ul>
      <h4 className="health-subtitle">{t("health.left.first")}</h4>
      <ol className="health-left-list">
        {ordered.slice(0, LEFT_ROWS).map((proposal) => (
          <li key={proposal.id} className="health-left-row">
            <a className="health-left-link" href={sectionHash(project, "inbox", proposal.id)}>
              <span className="mono">{proposal.id}</span>: <span>{summaryOf(proposal, t)}</span>
            </a>
            <span className="health-left-meta">
              <Badge name={t("health.left.severity")} look={severityLook(proposal.severity, t)} />
              <Stored label={t("health.left.created")} at={proposal.created_at} />
            </span>
          </li>
        ))}
      </ol>
    </>
  );
}

/**
 * Region 2, What is left (06 §7): the high-severity and the oldest proposals from the Inbox's read
 * (rows 1 and 2), the unapplied amendments, the code assuming a decision, the unbound nodes and the
 * drift from the health read (rows 3 to 6: UnappliedAmendments, AssumesPart, UnboundPart,
 * DriftPart), and a pointer to Budgets (row 7). One `getInbox` and the view's `getHealth`, each part
 * with its own states, boundary and Retry, which reads its read alone: a failing one leaves the
 * other standing, and one health row that cannot be measured leaves the others as they are.
 */
export function LeftRegion({ project, health }: { project: string; health: HealthQuery }) {
  const t = useTranslate();
  const inbox = useInbox(project);
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const healthFailure = useRetainedFailure(health.error, health.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(inbox.data, () => heading.current);
  const queueBusy = inbox.data === undefined ? failure === null : inbox.isFetching;
  // The health read's busy flag, computed here once: rows 3 to 6 show it, the region adds it to the queue's.
  const healthBusy = health.data === undefined ? healthFailure === null : health.isFetching;

  let queue;
  if (inbox.data === undefined) {
    queue =
      failure === null ? (
        <Skeleton label={t("health.left.queueLoading", { project })} lines={4} />
      ) : (
        <ReadFailure
          title={t("health.left.queueFailed")}
          failure={failure}
          retrying={inbox.isFetching}
          attempt={inbox.errorUpdateCount}
          onRetry={() => {
            retried();
            void inbox.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    queue = (
      <>
        <Queue project={project} answer={inbox.data} />
        {inbox.error !== null && !inbox.isFetching && (
          <p className="note" role="alert">
            {fill(t("health.left.queueFailedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(inbox.error).message}</span> })}
          </p>
        )}
      </>
    );
  }

  return (
    <HealthRegion
      title={t("health.left.title")}
      boundary={t("health.left.boundary")}
      headingRef={heading}
      busy={queueBusy || healthBusy}
      className="health-left"
      action={
        <a className="health-open" href={sectionHash(project, "inbox")}>
          {t("health.left.openInbox")}
          <Icon name="arrowRight" />
        </a>
      }
    >
      <div className="health-part" aria-busy={queueBusy}>
        <h3 className="health-part-title">{t("health.left.queueTitle")}</h3>
        <RegionBoundary name={t("health.left.queueBoundary")}>{queue}</RegionBoundary>
      </div>
      <UnappliedAmendments project={project} health={health} failure={healthFailure} busy={healthBusy} />
      <AssumesPart project={project} health={health} failure={healthFailure} busy={healthBusy} />
      <UnboundPart project={project} health={health} failure={healthFailure} busy={healthBusy} />
      <DriftPart project={project} health={health} failure={healthFailure} busy={healthBusy} />
      <p className="health-pointer">
        {fill(t("health.left.pointer", { region: slot("region") }), { region: <span className="health-pointer-name">{t("health.debt.title")}</span> })}
      </p>
    </HealthRegion>
  );
}
