import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useInbox } from "../api/queries";
import type { Inbox } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { statusLook } from "../inbox/labels";
import { queueOrder } from "../inbox/order";
import { summaryOf } from "../inbox/summary";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { Count, Notes, Region, Stored } from "./parts";
import { HOME_ROWS, kindCountsOf, statusCountsOf } from "./tally";

/**
 * One answer of the queue: its size, counts by status and by raw kind, and the first five in the
 * Inbox's order. Reads only the daemon's InboxEntry keys (docs/features/daemon-read.md "Data").
 */
function InboxSummary({ project, answer }: { project: string; answer: Inbox }) {
  const t = useTranslate();
  const notes = <Notes notes={answer.notes} label={t("home.inbox.notes")} />;
  const ordered = queueOrder(answer.proposals);
  if (ordered.length === 0) {
    return (
      <>
        {notes}
        <div className="home-empty">
          <p>{t("home.inbox.clear")}</p>
          <p>{fill(t("home.inbox.next", { link: slot("link") }), { link: <a href={sectionHash(project, "tasks")}>{t("home.inbox.nextLink")}</a> })}</p>
        </div>
      </>
    );
  }
  const first = ordered.slice(0, HOME_ROWS);
  return (
    <>
      {notes}
      <p className="home-total">{fill(t("home.inbox.total", { count: ordered.length, replace: { count: slot("count") } }), { count: <strong>{ordered.length}</strong> })}</p>
      <div className="home-split">
        <div className="home-block">
          <h3 className="home-block-title">{t("home.inbox.byStatus")}</h3>
          <ul className="home-tally">
            {statusCountsOf(ordered).map(({ status, count }) => (
              <li key={status}>
                <Badge look={statusLook(status, t)} />
                <Count value={count} />
              </li>
            ))}
          </ul>
        </div>
        <div className="home-block">
          <h3 className="home-block-title">{t("home.inbox.byKind")}</h3>
          <ul className="home-tally">
            {kindCountsOf(ordered).map(({ name, count }) => (
              <li key={name}>
                <span className="mono">{name}</span>
                <Count value={count} />
              </li>
            ))}
          </ul>
        </div>
      </div>
      <div className="home-block">
        <h3 className="home-block-title">{t("home.inbox.first")}</h3>
        <ul className="home-list">
          {first.map((proposal) => (
            <li key={proposal.id} className="home-row">
              <a className="home-row-link" href={sectionHash(project, "inbox", proposal.id)}>
                <span className="mono">{proposal.id}</span>: <span>{summaryOf(proposal)}</span>
              </a>
              <span className="home-row-meta">
                <Badge name={t("home.inbox.status")} look={statusLook(proposal.status, t)} />
                <Stored label={t("home.inbox.created")} at={proposal.created_at} />
              </span>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}

/**
 * The home's Inbox region: one `getInbox` per visit, its own loading, empty and error states, and a
 * Retry that reads the queue again and nothing else.
 */
export function InboxRegion({ project }: { project: string }) {
  const t = useTranslate();
  const inbox = useInbox(project);
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(inbox.data, () => heading.current);

  let body;
  if (inbox.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("home.inbox.loading", { project })} lines={4} />
      ) : (
        <ErrorPanel
          title={t("home.inbox.failed")}
          message={apiErrorOf(failure).message}
          retrying={inbox.isFetching}
          attempt={inbox.errorUpdateCount}
          onRetry={() => {
            retried();
            void inbox.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = (
      <>
        <InboxSummary project={project} answer={inbox.data} />
        {inbox.error !== null && (
          <p className="note" role="alert">
            {fill(t("home.inbox.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(inbox.error).message}</span> })}
          </p>
        )}
      </>
    );
  }

  return (
    <Region
      title={t("home.inbox.title")}
      boundary={t("home.inbox.boundary")}
      headingRef={heading}
      busy={inbox.data === undefined && failure === null}
      link={
        <a className="home-open" href={sectionHash(project, "inbox")}>
          {t("home.inbox.open")}
          <Icon name="arrowRight" />
        </a>
      }
    >
      {body}
    </Region>
  );
}
