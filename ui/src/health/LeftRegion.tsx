import { useRef, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useInbox } from "../api/queries";
import type { Inbox } from "../api/types";
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
import { severityCountsOf } from "./findings";
import { HealthRegion } from "./parts";

/** How many proposals of the queue are listed, in the Inbox's order. */
export const LEFT_ROWS = 5;

/**
 * Rows 3 to 6 of "what is left" (06 §7), which nothing measures yet: each says so and names its
 * source; never a number (docs/features/ui-health.md "Description and interactions").
 */
const UNMEASURED: readonly (readonly [key: string, what: ReactNode, source: ReactNode])[] = [
  [
    "amendments",
    "Unapplied amendments",
    <>
      the <code>amendment</code> kind of <code>proposal-kinds</code>
    </>,
  ],
  [
    "assumes",
    <>
      <code>@assumes</code> against a decision
    </>,
    "Phase 3",
  ],
  [
    "unbound",
    <>
      <code>unbound</code> accepted nodes
    </>,
    "Phase 3",
  ],
  ["drift", "Drift", "Phase 3"],
];

/** Rows 1 and 2: the queue by severity, then its first five as the Inbox lists them. */
function Queue({ project, answer }: { project: string; answer: Inbox }) {
  const ordered = queueOrder(answer.proposals);
  const notes = <Notes notes={answer.notes} label="Notes from the daemon on the queue" />;
  if (ordered.length === 0) {
    return (
      <>
        {notes}
        <p className="health-left-total">No proposal waits for your decision.</p>
      </>
    );
  }
  return (
    <>
      {notes}
      <p className="health-left-total">
        <strong>{ordered.length}</strong> {ordered.length === 1 ? "proposal" : "proposals"} in the queue
      </p>
      <ul className="health-tally" aria-label="Open proposals by severity">
        {severityCountsOf(ordered).map(({ severity, count }) => (
          <li key={severity ?? ""}>
            <Badge look={severityLook(severity)} />
            <span className="sr-only">:</span> <span className="health-tally-count">{count}</span>
          </li>
        ))}
      </ul>
      <h4 className="health-subtitle">First in the queue</h4>
      <ol className="health-left-list">
        {ordered.slice(0, LEFT_ROWS).map((proposal) => (
          <li key={proposal.id} className="health-left-row">
            <a className="health-left-link" href={sectionHash(project, "inbox", proposal.id)}>
              <span className="mono">{proposal.id}</span>: <span>{summaryOf(proposal)}</span>
            </a>
            <span className="health-left-meta">
              <Badge name="Severity" look={severityLook(proposal.severity)} />
              <Stored label="Created" at={proposal.created_at} />
            </span>
          </li>
        ))}
      </ol>
    </>
  );
}

/**
 * Region 2, What is left (06 §7): the high-severity and the oldest proposals from the Inbox's read,
 * the rows nothing measures yet, and a pointer to Budgets. One `getInbox`; its Retry reads it alone.
 */
export function LeftRegion({ project }: { project: string }) {
  const inbox = useInbox(project);
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(inbox.data, () => heading.current);

  let queue;
  if (inbox.data === undefined) {
    queue =
      failure === null ? (
        <Skeleton label={`Loading the queue of ${project}`} lines={4} />
      ) : (
        <ReadFailure
          title="The queue could not be loaded"
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
            The queue could not be read again: {apiErrorOf(inbox.error).message}
          </p>
        )}
      </>
    );
  }

  return (
    <HealthRegion
      title="What is left"
      boundary="list of what is left"
      headingRef={heading}
      busy={inbox.data === undefined ? failure === null : inbox.isFetching}
      className="health-left"
      action={
        <a className="health-open" href={sectionHash(project, "inbox")}>
          Open Inbox
          <Icon name="arrowRight" />
        </a>
      }
    >
      <div className="health-part">
        <h3 className="health-part-title">Proposals in the queue</h3>
        {queue}
      </div>
      <div className="health-part">
        <h3 className="health-part-title">Not measured yet</h3>
        <dl className="health-unmeasured">
          {UNMEASURED.map(([key, what, source]) => (
            <div key={key} className="health-unmeasured-row" data-row={key}>
              <dt>{what}</dt>
              <dd>
                <span className="health-value">Not measured yet</span>: <span className="health-source">{source}</span>
              </dd>
            </div>
          ))}
        </dl>
      </div>
      <p className="health-pointer">
        Documents over their budget: see Budgets, under <span className="health-pointer-name">Debt and budgets</span>.
      </p>
    </HealthRegion>
  );
}
