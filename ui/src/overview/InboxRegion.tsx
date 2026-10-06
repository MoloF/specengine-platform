import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useInbox } from "../api/queries";
import type { Inbox } from "../api/types";
import { sectionHash } from "../app/routes";
import { statusLook } from "../inbox/labels";
import { queueOrder } from "../inbox/order";
import { summaryOf } from "../inbox/summary";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { Count, Notes, Region, Stored } from "./parts";
import { HOME_ROWS, kindCountsOf, proposalNoun, statusCountsOf } from "./tally";

/**
 * One answer of the queue: its size, counts by status and by raw kind, and the first five in the
 * Inbox's order. Reads only the daemon's InboxEntry keys (docs/features/daemon-read.md "Data").
 */
function InboxSummary({ project, answer }: { project: string; answer: Inbox }) {
  const notes = <Notes notes={answer.notes} label="Notes from the daemon on the queue" />;
  const ordered = queueOrder(answer.proposals);
  if (ordered.length === 0) {
    return (
      <>
        {notes}
        <div className="home-empty">
          <p>The queue is clear: no proposal waits for your decision.</p>
          <p>
            Next: <a href={sectionHash(project, "tasks")}>approve the tasks that are ready for development</a>.
          </p>
        </div>
      </>
    );
  }
  const first = ordered.slice(0, HOME_ROWS);
  return (
    <>
      {notes}
      <p className="home-total">
        <strong>{ordered.length}</strong> {proposalNoun(ordered.length)} in the queue
      </p>
      <div className="home-split">
        <div className="home-block">
          <h3 className="home-block-title">By status</h3>
          <ul className="home-tally">
            {statusCountsOf(ordered).map(({ status, count }) => (
              <li key={status}>
                <Badge look={statusLook(status)} />
                <Count value={count} />
              </li>
            ))}
          </ul>
        </div>
        <div className="home-block">
          <h3 className="home-block-title">By kind</h3>
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
        <h3 className="home-block-title">First in the queue</h3>
        <ul className="home-list">
          {first.map((proposal) => (
            <li key={proposal.id} className="home-row">
              <a className="home-row-link" href={sectionHash(project, "inbox", proposal.id)}>
                <span className="mono">{proposal.id}</span>: <span>{summaryOf(proposal)}</span>
              </a>
              <span className="home-row-meta">
                <Badge name="Status" look={statusLook(proposal.status)} />
                <Stored label="Created" at={proposal.created_at} />
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
  const inbox = useInbox(project);
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(inbox.data, () => heading.current);

  let body;
  if (inbox.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={`Loading the inbox of ${project}`} lines={4} />
      ) : (
        <ErrorPanel
          title="The inbox could not be loaded"
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
            The inbox could not be read again: {apiErrorOf(inbox.error).message}
          </p>
        )}
      </>
    );
  }

  return (
    <Region
      title="Inbox"
      boundary="inbox overview"
      headingRef={heading}
      busy={inbox.data === undefined && failure === null}
      link={
        <a className="home-open" href={sectionHash(project, "inbox")}>
          Open Inbox
          <Icon name="arrowRight" />
        </a>
      }
    >
      {body}
    </Region>
  );
}
